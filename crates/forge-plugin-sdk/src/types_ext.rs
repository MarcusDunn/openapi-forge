//! IR type-traversal helpers shared across generators.
//!
//! Issue #107 dropped the per-`TypeDef` `nullable` flag in favour of a
//! `TypeDef::Null` unit type plus the convention that `T | null` is a
//! `Union` with two variants (Null canonicalized last). These helpers
//! peel that representation back into the "is this nullable, and what's
//! the inner type" question generators actually want to ask.

use crate::ir::{NamedType, TypeDef, TypeRef, UnionType, NULL_ID};

/// True when `r` resolves (in `types`) to the canonical [`TypeDef::Null`]
/// singleton or any other [`TypeDef::Null`] entry.
pub fn is_null_typeref(types: &[NamedType], r: &TypeRef) -> bool {
    types
        .iter()
        .any(|nt| nt.id == *r && matches!(nt.definition, TypeDef::Null))
}

/// If `r` resolves to a two-variant `Union` whose other variant is the
/// `Null` singleton, returns the inner non-null TypeRef. The parser
/// canonicalises every `T | null` to that exact shape (issue #107), so a
/// peel+wrap round-trip is the generator's standard "render `T?`" path.
///
/// Returns `None` for:
/// - non-Union TypeRefs
/// - Unions with more than two variants (e.g. `string | int | null`) —
///   the caller emits its full union machinery and consults
///   [`union_has_null`] to decide whether to wrap consumers in
///   `Option`/`| null`.
/// - the bare Null singleton itself
pub fn peel_nullable<'a>(types: &'a [NamedType], r: &TypeRef) -> Option<&'a TypeRef> {
    let nt = types.iter().find(|nt| &nt.id == r)?;
    let TypeDef::Union(u) = &nt.definition else {
        return None;
    };
    if u.variants.len() != 2 {
        return None;
    }
    let null_idx = u
        .variants
        .iter()
        .position(|v| is_null_typeref(types, &v.r#type))?;
    let other_idx = if null_idx == 0 { 1 } else { 0 };
    Some(&u.variants[other_idx].r#type)
}

/// True when `u`'s variants list contains a reference to the Null
/// singleton. Useful for multi-variant unions the simple
/// [`peel_nullable`] doesn't unwrap.
pub fn union_has_null(types: &[NamedType], u: &UnionType) -> bool {
    u.variants.iter().any(|v| is_null_typeref(types, &v.r#type))
}

/// `true` when `r` is the canonical Null singleton id (`"null"`).
pub fn is_canonical_null_id(r: &TypeRef) -> bool {
    r == NULL_ID
}
