//! `$ref` resolution for the narrow Stage 3 subset.
//!
//! Only local references into `#/components/schemas/<Name>` are supported.
//! External references and refs into anything other than `components/schemas`
//! produce a diagnostic.

/// Outcome of resolving a `$ref` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RefOutcome {
    /// Successfully resolved to a component schema name (sanitized id).
    Component(String),
    /// External reference (URI/relative path with non-empty fragment-less prefix).
    External,
    /// Local reference but not into `#/components/schemas/`.
    UnsupportedLocal,
    /// Reference into `components/schemas` but the schema is not declared.
    Dangling(String),
}

/// Set of declared component-schema ids (sanitized). Built up-front so
/// forward references resolve cleanly.
#[derive(Debug, Default, Clone)]
pub(crate) struct RefIndex {
    schemas: indexmap::IndexSet<String>,
}

impl RefIndex {
    pub fn register(&mut self, id: impl Into<String>) {
        self.schemas.insert(id.into());
    }

    pub fn contains(&self, id: &str) -> bool {
        self.schemas.contains(id)
    }
}

/// Resolve a `$ref` string against the current document's `RefIndex`.
///
/// Supports two local fragment shapes:
/// - `#/components/schemas/<Name>` — the standard wrapped form.
/// - `#/<Name>` — the flat-root form used by split-document specs
///   where the file is itself the schema map.
///
/// Anything else local that the index doesn't recognise becomes
/// `UnsupportedLocal` (the fragment doesn't address a known schema).
/// Cross-file refs (anything before `#`) become `External`.
pub(crate) fn resolve(index: &RefIndex, raw: &str) -> RefOutcome {
    // External: anything that does not start with `#`.
    let Some(fragment) = raw.strip_prefix('#') else {
        return RefOutcome::External;
    };
    let path = fragment.strip_prefix('/').unwrap_or(fragment);
    // Standard wrapped form first.
    if let Some(name) = path.strip_prefix("components/schemas/") {
        let decoded = decode_pointer_token(name);
        let id = crate::sanitize::ident(&decoded);
        return if index.contains(&id) {
            RefOutcome::Component(id)
        } else {
            RefOutcome::Dangling(id)
        };
    }
    // Flat-root form: a single token addressing a registered schema name.
    if !path.is_empty() && !path.contains('/') {
        let decoded = decode_pointer_token(path);
        let id = crate::sanitize::ident(&decoded);
        if index.contains(&id) {
            return RefOutcome::Component(id);
        }
    }
    RefOutcome::UnsupportedLocal
}

fn decode_pointer_token(s: &str) -> String {
    // Order matters: ~1 first, then ~0 (per RFC 6901).
    s.replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idx_with(names: &[&str]) -> RefIndex {
        let mut i = RefIndex::default();
        for n in names {
            i.register(*n);
        }
        i
    }

    #[test]
    fn component_ref() {
        let idx = idx_with(&["Pet"]);
        assert_eq!(
            resolve(&idx, "#/components/schemas/Pet"),
            RefOutcome::Component("Pet".into())
        );
    }

    #[test]
    fn dangling_ref() {
        let idx = idx_with(&["Pet"]);
        assert_eq!(
            resolve(&idx, "#/components/schemas/Missing"),
            RefOutcome::Dangling("Missing".into())
        );
    }

    #[test]
    fn external_ref() {
        let idx = RefIndex::default();
        assert_eq!(
            resolve(&idx, "other.json#/components/schemas/Pet"),
            RefOutcome::External
        );
    }

    #[test]
    fn unsupported_local_ref() {
        let idx = RefIndex::default();
        assert_eq!(
            resolve(&idx, "#/components/parameters/Foo"),
            RefOutcome::UnsupportedLocal
        );
    }

    #[test]
    fn pointer_decoding() {
        let idx = idx_with(&["a_b_c"]); // sanitized form of "a/b~c"
        assert_eq!(
            resolve(&idx, "#/components/schemas/a~1b~0c"),
            RefOutcome::Component("a_b_c".into())
        );
    }
}
