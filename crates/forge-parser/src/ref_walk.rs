//! Generic `$ref` resolution for non-schema OAS objects.
//!
//! Schema refs go through [`crate::schema::walk_external_ref`] because
//! they need to register a `NamedType` in the type pool. Path-items,
//! parameters, responses, request-bodies, and security-schemes don't
//! produce IR types — they just need their inline value walked. This
//! module is the shared dispatcher.

use std::path::PathBuf;

use serde_json::Value as J;

use crate::ctx::Ctx;
use crate::diag;
use crate::external::{resolve_pointer, split_ref};
use crate::pointer::Ptr;

/// If `value` is `{$ref: ...}`, resolve the ref (following chains of
/// refs through any number of documents) and run `body` with the final
/// inline target value. `ctx.current_doc` is switched to the target
/// document's canonical path while `body` runs and restored on the way
/// out. If `value` is not a ref, `body` runs with it directly.
///
/// `body` returns `Some(_)` on a successful walk; the caller's diagnostic
/// flow takes over from there. Cycles produce `parser/E-CYCLIC-REF` —
/// non-schema cycles can't form legitimate IR.
pub(crate) fn with_resolved_object<T, F>(
    ctx: &mut Ctx,
    value: &J,
    ptr: &mut Ptr,
    body: F,
) -> Option<T>
where
    F: FnOnce(&mut Ctx, &J, &mut Ptr) -> Option<T>,
{
    let mut cursor = value.clone();
    let mut pushed: Vec<(PathBuf, String)> = Vec::new();
    let prev_doc = ctx.current_doc.clone();
    let mut ok = true;
    // OAS 3.1+ allows sibling keywords on `$ref` (and OAS 3.2 codifies
    // it for non-schema Reference Objects: `summary` / `description`).
    // Snapshot the immediate parent object's siblings *before* chain-
    // walking so we can overlay them onto the final resolved value.
    // 3.0 specs drop siblings with `parser/W-REF-SIBLINGS-3-0` (handled
    // below).
    let initial_siblings: Option<serde_json::Map<String, J>> = if !ctx.is_oas_3_0 {
        cursor.as_object().and_then(|m| {
            if !m.contains_key("$ref") {
                return None;
            }
            let mut sibs = serde_json::Map::new();
            for (k, v) in m {
                if k == "$ref" || k.starts_with("x-") {
                    continue;
                }
                sibs.insert(k.clone(), v.clone());
            }
            (!sibs.is_empty()).then_some(sibs)
        })
    } else {
        // 3.0: warn-and-drop. Existing W_REF_SIBLINGS_3_0 path lives
        // in `crate::schema`; non-schema refs in 3.0 just drop them
        // silently (#74 only wired up the schema-side warning).
        None
    };
    while let Some(raw_ref) = cursor
        .as_object()
        .and_then(|m| m.get("$ref"))
        .and_then(|r| r.as_str())
        .map(str::to_string)
    {
        let (file_part, fragment) = split_ref(&raw_ref);

        // Resolve to a canonical path (possibly switching docs).
        let canonical = if file_part.is_empty() {
            ctx.current_doc.clone()
        } else {
            let from = ctx.current_doc.clone();
            let loaded = match ctx.resolver.load(&raw_ref, &from) {
                Ok(d) => d,
                Err(e) => {
                    ctx.push_diag(diag::err(
                        crate::diag::E_EXTERNAL_REF,
                        crate::schema::resolver_error_message(&raw_ref, &e),
                        ptr.loc(ctx.file),
                    ));
                    ok = false;
                    break;
                }
            };
            let canonical = loaded.canonical_path.clone();
            crate::schema::ensure_doc_registered(ctx, &canonical, &loaded.root);
            canonical
        };

        let Some(root) = ctx.doc_roots.get(&canonical).cloned() else {
            ctx.push_diag(diag::err(
                crate::diag::E_EXTERNAL_REF,
                format!("internal: doc `{}` not in cache", canonical.display()),
                ptr.loc(ctx.file),
            ));
            ok = false;
            break;
        };

        let Some(target) = resolve_pointer(&root, fragment) else {
            ctx.push_diag(diag::err(
                crate::diag::E_DANGLING_REF,
                format!(
                    "$ref `{raw_ref}` does not resolve against `{}`",
                    canonical.display()
                ),
                ptr.loc(ctx.file),
            ));
            ok = false;
            break;
        };

        // Track resolved refs into the main spec's
        // `components.pathItems` so the unused-declaration warning at
        // the end of parse can tell which were touched. Only the main
        // spec gets this treatment — external-doc pathItems aren't
        // declared in the document we own.
        let main_doc = ctx
            .doc_prefix
            .iter()
            .find(|(_, prefix)| prefix.is_empty())
            .map(|(p, _)| p.clone());
        if Some(&canonical) == main_doc.as_ref() {
            if let Some(name) = fragment.strip_prefix("/components/pathItems/") {
                ctx.referenced_component_path_items.insert(name.to_string());
            }
            if let Some(name) = fragment.strip_prefix("/components/mediaTypes/") {
                ctx.referenced_component_media_types
                    .insert(name.to_string());
            }
        }

        let walking_key = (canonical.clone(), fragment.to_string());
        if ctx.walking.contains(&walking_key) {
            ctx.push_diag(diag::err(
                crate::diag::E_CYCLIC_REF,
                format!("$ref `{raw_ref}` forms a cycle"),
                ptr.loc(ctx.file),
            ));
            ok = false;
            break;
        }
        ctx.walking.insert(walking_key.clone());
        pushed.push(walking_key);
        ctx.current_doc = canonical;
        cursor = target.clone();
    }

    // 3.1+ sibling merge: siblings on the `$ref` source object win
    // over the resolved target's same-keyed fields. Spec ref:
    // OAS 3.2 §3.5, JSON Schema 2020-12 §8.2.3.
    let merged = if ok && initial_siblings.is_some() && cursor.is_object() {
        let mut m = cursor.as_object().cloned().unwrap_or_default();
        if let Some(sibs) = initial_siblings {
            for (k, v) in sibs {
                m.insert(k, v);
            }
        }
        J::Object(m)
    } else {
        cursor
    };

    let result = if ok { body(ctx, &merged, ptr) } else { None };

    for k in pushed.iter().rev() {
        ctx.walking.remove(k);
    }
    ctx.current_doc = prev_doc;
    result
}
