//! The public JSON Schema for the plan artifact.
//!
//! The full schema is derived solely from [`crate::document::RawPlanDocument`],
//! the one and only deserialization source. There is no schema version and no
//! second schema.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("unknown schema section `{name}`; available sections: {}", available.join(", "))]
    UnknownSection {
        name: String,
        available: Vec<String>,
    },
}

/// The full plan schema, derived from `RawPlanDocument`.
pub fn full() -> serde_json::Value {
    crate::document::schema()
}

pub fn section_names() -> Vec<String> {
    let schema = full();
    schema
        .get("$defs")
        .and_then(serde_json::Value::as_object)
        .map(|defs| defs.keys().cloned().collect())
        .unwrap_or_default()
}

pub fn section(name: &str) -> Result<serde_json::Value, SchemaError> {
    let schema = full();
    schema
        .get("$defs")
        .and_then(|defs| defs.get(name))
        .cloned()
        .ok_or_else(|| SchemaError::UnknownSection {
            name: name.to_owned(),
            available: section_names(),
        })
}
