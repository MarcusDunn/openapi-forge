//! Conversions from `serde_json::Value` into the IR's value pool.
//!
//! `forge_ir::Value` carries compound arms (`List`, `Object`) whose
//! contents are `ValueRef` indices into [`forge_ir::Ir::values`]. The
//! parser interns every `serde_json::Value` it sees through
//! [`intern`] / [`intern_into`], which walks the tree, pushes the leaf
//! and compound nodes into a [`ValuePool`], and returns the
//! `ValueRef` of the root.
//!
//! Structural deduplication: pushing a `Value` that already exists at
//! index `i` returns `i` (saves space when many properties default to
//! `42` or `null`). Float dedup uses bitwise equality (`f64::to_bits`)
//! so NaN values are not deduped — which is the desired behaviour.

use forge_ir::{Value, ValueRef};
use serde_json::Value as J;
use std::collections::HashMap;

/// A growable pool of `Value`s with structural deduplication. Owned by
/// the parser's [`crate::ctx::Ctx`]. Finalised into [`forge_ir::Ir::values`].
#[derive(Debug, Default)]
pub(crate) struct ValuePool {
    nodes: Vec<Value>,
    by_node: HashMap<NodeKey, ValueRef>,
}

impl ValuePool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a `Value` (or return its existing index if structurally
    /// equal to an entry already in the pool). Compound arms must
    /// already point at valid indices — [`intern_json`] is the typical
    /// caller and handles the recursion.
    pub fn intern(&mut self, v: Value) -> ValueRef {
        let key = NodeKey::from(&v);
        if let Some(&i) = self.by_node.get(&key) {
            return i;
        }
        let idx: ValueRef = self.nodes.len().try_into().expect("value pool overflow");
        self.nodes.push(v);
        self.by_node.insert(key, idx);
        idx
    }

    /// Walk a `serde_json::Value` recursively, interning every leaf and
    /// compound node, and return the root's `ValueRef`.
    pub fn intern_json(&mut self, v: &J) -> ValueRef {
        let node = match v {
            J::Null => Value::Null,
            J::Bool(b) => Value::Bool { value: *b },
            J::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Int { value: i }
                } else if let Some(u) = n.as_u64() {
                    if u <= i64::MAX as u64 {
                        Value::Int { value: u as i64 }
                    } else {
                        Value::Float { value: u as f64 }
                    }
                } else {
                    Value::Float {
                        value: n.as_f64().unwrap_or(0.0),
                    }
                }
            }
            J::String(s) => Value::String { value: s.clone() },
            J::Array(items) => {
                let refs: Vec<ValueRef> = items.iter().map(|x| self.intern_json(x)).collect();
                Value::List { items: refs }
            }
            J::Object(map) => {
                let fields: Vec<(String, ValueRef)> = map
                    .iter()
                    .map(|(k, x)| (k.clone(), self.intern_json(x)))
                    .collect();
                Value::Object { fields }
            }
        };
        self.intern(node)
    }

    pub fn finish(self) -> Vec<Value> {
        self.nodes
    }
}

/// Hashable wrapper around `Value`. `f64` doesn't implement `Hash`
/// natively (NaN comparisons are tricky); we hash by bit pattern via
/// `f64::to_bits`, which means NaN values won't dedup against each
/// other (but won't crash either).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NodeKey {
    Null,
    Bool(bool),
    Int(i64),
    FloatBits(u64),
    String(String),
    List(Vec<ValueRef>),
    Object(Vec<(String, ValueRef)>),
}

impl From<&Value> for NodeKey {
    fn from(v: &Value) -> Self {
        match v {
            Value::Null => NodeKey::Null,
            Value::Bool { value } => NodeKey::Bool(*value),
            Value::Int { value } => NodeKey::Int(*value),
            Value::Float { value } => NodeKey::FloatBits(value.to_bits()),
            Value::String { value } => NodeKey::String(value.clone()),
            Value::List { items } => NodeKey::List(items.clone()),
            Value::Object { fields } => NodeKey::Object(fields.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_scalars_dedup() {
        let mut p = ValuePool::new();
        let a = p.intern_json(&J::from(42));
        let b = p.intern_json(&J::from(42));
        assert_eq!(a, b);
        assert_eq!(p.finish().len(), 1);
    }

    #[test]
    fn intern_compound_object_walks_recursively() {
        let mut p = ValuePool::new();
        let v: J = serde_json::from_str(r#"{"a": [1, 2], "b": null}"#).unwrap();
        let _root = p.intern_json(&v);
        let pool = p.finish();
        // 1, 2, [1,2], null, {a, b}. Pool has unique scalars (1, 2,
        // null) plus the list and the object — 5 entries.
        assert_eq!(pool.len(), 5);
    }

    #[test]
    fn intern_object_with_duplicate_value_dedups() {
        let mut p = ValuePool::new();
        let v: J = serde_json::from_str(r#"{"a": 1, "b": 1}"#).unwrap();
        let _ = p.intern_json(&v);
        let pool = p.finish();
        // The shared `1` is interned once; the object is the second
        // entry.
        assert_eq!(pool.len(), 2);
    }
}
