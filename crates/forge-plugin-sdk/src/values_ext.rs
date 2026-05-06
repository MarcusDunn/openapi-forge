//! Helpers for resolving and rendering pool-stored [`Value`]s.
//!
//! IR fields hold a [`ValueRef`] (a `u32` index into [`Ir::values`]).
//! Compound `Value::List` / `Value::Object` arms hold *more* `ValueRef`s
//! into the same pool — see ADR-0007. These helpers walk the pool to
//! materialise tree-shaped representations on demand.

use crate::ir::{Value, ValueRef};

/// One-step deref. Returns the pool node at `r`. Returns `None` only
/// when `r` is out of bounds — IR validation is the host's job; plugins
/// should treat `None` as a programmer error.
pub fn resolve(values: &[Value], r: ValueRef) -> Option<&Value> {
    values.get(r as usize)
}

/// Recursive resolution into a tree-shaped [`serde_json::Value`].
/// Plugins that need to print, hash, or otherwise consume the full tree
/// of a value (e.g. for example doc-comments) use this.
pub fn resolve_to_serde(values: &[Value], r: ValueRef) -> serde_json::Value {
    let Some(node) = values.get(r as usize) else {
        return serde_json::Value::Null;
    };
    match node {
        Value::Null => serde_json::Value::Null,
        Value::Bool { value } => serde_json::Value::Bool(*value),
        Value::Int { value } => serde_json::Value::Number((*value).into()),
        Value::Float { value } => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::String { value } => serde_json::Value::String(value.clone()),
        Value::List { items } => serde_json::Value::Array(
            items
                .iter()
                .map(|i| resolve_to_serde(values, *i))
                .collect(),
        ),
        Value::Object { fields } => {
            let mut map = serde_json::Map::with_capacity(fields.len());
            for (k, v) in fields {
                map.insert(k.clone(), resolve_to_serde(values, *v));
            }
            serde_json::Value::Object(map)
        }
    }
}

/// Render a value as a compact JSON string, suitable for inline
/// doc-comment emission.
pub fn to_json_compact(values: &[Value], r: ValueRef) -> String {
    serde_json::to_string(&resolve_to_serde(values, r)).unwrap_or_default()
}

/// Render a value as a pretty-printed JSON string.
pub fn to_json_pretty(values: &[Value], r: ValueRef) -> String {
    serde_json::to_string_pretty(&resolve_to_serde(values, r)).unwrap_or_default()
}
