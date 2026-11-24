//! An RFC 6902 `add`/`replace`/`remove` subset over `serde_json::Value`.
//!
//! A correction or an exploration result can be supplied as a patch against the
//! packet's subject document instead of a full result (design §10). Only the three
//! ops are supported — `move`/`copy`/`test` are out of scope — and navigation uses
//! `serde_json`'s own JSON-pointer semantics, with array `-` appending to the end.

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

/// One RFC 6902 operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchOp {
    Add,
    Replace,
    Remove,
}

impl PatchOp {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Replace => "replace",
            Self::Remove => "remove",
        }
    }
}

/// One patch operation: the op, the JSON pointer it targets, and (for add/replace)
/// the value to place there.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchOperation {
    pub op: PatchOp,
    pub path: String,
    #[serde(default)]
    pub value: Option<Value>,
}

/// A patch operation that could not be applied, located by its index and pointer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("patch op {index} (`{op} {path}`) failed: {detail}")]
pub struct PatchError {
    pub index: usize,
    pub op: &'static str,
    pub path: String,
    pub detail: String,
}

/// Apply every operation to `document` in order, stopping at the first failure.
pub fn apply_all(document: &mut Value, operations: &[PatchOperation]) -> Result<(), PatchError> {
    for (index, operation) in operations.iter().enumerate() {
        apply_one(document, operation).map_err(|detail| PatchError {
            index,
            op: operation.op.name(),
            path: operation.path.clone(),
            detail,
        })?;
    }
    Ok(())
}

fn apply_one(document: &mut Value, operation: &PatchOperation) -> Result<(), String> {
    match operation.op {
        PatchOp::Replace => {
            let value = operation
                .value
                .clone()
                .ok_or_else(|| "replace requires a value".to_owned())?;
            let target = document
                .pointer_mut(&operation.path)
                .ok_or_else(|| "path does not exist".to_owned())?;
            *target = value;
            Ok(())
        }
        PatchOp::Add => {
            let value = operation
                .value
                .clone()
                .ok_or_else(|| "add requires a value".to_owned())?;
            add(document, &operation.path, value)
        }
        PatchOp::Remove => remove(document, &operation.path),
    }
}

/// Split a JSON pointer into its parent pointer and its unescaped last token.
fn split(path: &str) -> Result<(&str, String), String> {
    let (parent, token) = path
        .rsplit_once('/')
        .ok_or_else(|| "path must be a JSON pointer with at least one segment".to_owned())?;
    Ok((parent, unescape(token)))
}

/// Undo the JSON-pointer escaping: `~1` is `/`, `~0` is `~` (in that order).
fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

fn add(document: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let (parent_pointer, token) = split(path)?;
    let parent = document
        .pointer_mut(parent_pointer)
        .ok_or_else(|| "parent path does not exist".to_owned())?;
    match parent {
        Value::Object(map) => {
            map.insert(token, value);
            Ok(())
        }
        Value::Array(array) => {
            if token == "-" {
                array.push(value);
                return Ok(());
            }
            let index = token
                .parse::<usize>()
                .map_err(|_| "array index is not a number".to_owned())?;
            if index > array.len() {
                return Err("array index is out of bounds".to_owned());
            }
            array.insert(index, value);
            Ok(())
        }
        _ => Err("parent is neither an object nor an array".to_owned()),
    }
}

fn remove(document: &mut Value, path: &str) -> Result<(), String> {
    let (parent_pointer, token) = split(path)?;
    let parent = document
        .pointer_mut(parent_pointer)
        .ok_or_else(|| "parent path does not exist".to_owned())?;
    match parent {
        Value::Object(map) => map
            .remove(&token)
            .map(|_| ())
            .ok_or_else(|| "key does not exist".to_owned()),
        Value::Array(array) => {
            let index = token
                .parse::<usize>()
                .map_err(|_| "array index is not a number".to_owned())?;
            if index >= array.len() {
                return Err("array index is out of bounds".to_owned());
            }
            array.remove(index);
            Ok(())
        }
        _ => Err("parent is neither an object nor an array".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    fn ops(value: Value) -> Vec<PatchOperation> {
        serde_json::from_value(value).expect("valid ops")
    }

    #[test]
    fn add_inserts_an_object_key() {
        let mut document = json!({"a": 1});
        apply_all(
            &mut document,
            &ops(json!([{"op": "add", "path": "/b", "value": 2}])),
        )
        .unwrap();
        assert_eq!(document, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn add_dash_appends_to_an_array() {
        let mut document = json!({"xs": [1, 2]});
        apply_all(
            &mut document,
            &ops(json!([{"op": "add", "path": "/xs/-", "value": 3}])),
        )
        .unwrap();
        assert_eq!(document, json!({"xs": [1, 2, 3]}));
    }

    #[test]
    fn replace_overwrites_an_existing_value() {
        let mut document = json!({"a": 1});
        apply_all(
            &mut document,
            &ops(json!([{"op": "replace", "path": "/a", "value": 9}])),
        )
        .unwrap();
        assert_eq!(document, json!({"a": 9}));
    }

    #[test]
    fn remove_deletes_a_key() {
        let mut document = json!({"a": 1, "b": 2});
        apply_all(&mut document, &ops(json!([{"op": "remove", "path": "/a"}]))).unwrap();
        assert_eq!(document, json!({"b": 2}));
    }

    #[test]
    fn a_bad_pointer_is_reported_with_its_index() {
        let mut document = json!({"a": 1});
        let error = apply_all(
            &mut document,
            &ops(json!([{"op": "replace", "path": "/missing", "value": 1}])),
        )
        .expect_err("missing path fails");
        assert_eq!(error.index, 0);
        assert_eq!(error.op, "replace");
    }
}
