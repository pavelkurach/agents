//! Reading and atomically persisting the proof-carrying plan document.
//!
//! The artifact layer is the sole boundary between raw bytes on disk and the
//! proven [`ParsedPlan`]: [`load`] parses once, and [`create_new`] /
//! [`save_atomic`] write a plan's canonical serialization through a temp file so
//! a reader never observes a half-written artifact.

use std::io::Write;
use std::path::Path;

use thiserror::Error;

use crate::document::{ParsePlanError, ParsedPlan};

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} already exists")]
    Exists { path: String },
    #[error("{path} could not be serialized: {source}")]
    Serialize {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path} is not a valid plan artifact: {source}")]
    Parse {
        path: String,
        #[source]
        source: ParsePlanError,
    },
}

fn canonical(path: &Path, plan: &ParsedPlan) -> Result<String, ArtifactError> {
    plan.to_canonical_json()
        .map_err(|source| ArtifactError::Serialize {
            path: path.display().to_string(),
            source,
        })
}

fn write_tempfile(
    parent: &Path,
    rendered: &[u8],
    display_path: &Path,
) -> Result<tempfile::NamedTempFile, ArtifactError> {
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|source| ArtifactError::Io {
        path: parent.display().to_string(),
        source,
    })?;
    temp.write_all(rendered)
        .and_then(|()| temp.as_file().sync_all())
        .map_err(|source| ArtifactError::Io {
            path: display_path.display().to_string(),
            source,
        })?;
    Ok(temp)
}

fn persist_replace(path: &Path, rendered: &[u8]) -> Result<(), ArtifactError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ArtifactError::Io {
        path: parent.display().to_string(),
        source,
    })?;
    let temp = write_tempfile(parent, rendered, path)?;
    temp.persist(path).map_err(|error| ArtifactError::Io {
        path: path.display().to_string(),
        source: error.error,
    })?;
    Ok(())
}

fn persist_new(path: &Path, rendered: &[u8]) -> Result<(), ArtifactError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|source| ArtifactError::Io {
        path: parent.display().to_string(),
        source,
    })?;
    let temp = write_tempfile(parent, rendered, path)?;
    temp.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            ArtifactError::Exists {
                path: path.display().to_string(),
            }
        } else {
            ArtifactError::Io {
                path: path.display().to_string(),
                source: error.error,
            }
        }
    })?;
    Ok(())
}

/// Read and parse a plan artifact into its proven form.
pub fn load(path: &Path) -> Result<ParsedPlan, ArtifactError> {
    let raw = std::fs::read(path).map_err(|source| ArtifactError::Io {
        path: path.display().to_string(),
        source,
    })?;
    ParsedPlan::parse(&raw).map_err(|source| ArtifactError::Parse {
        path: path.display().to_string(),
        source,
    })
}

/// Write a new plan artifact, failing if one already exists at the path.
pub fn create_new(path: &Path, plan: &ParsedPlan) -> Result<(), ArtifactError> {
    let rendered = canonical(path, plan)?;
    persist_new(path, rendered.as_bytes())
}

/// Atomically replace the plan artifact with the canonical serialization of `plan`.
pub fn save_atomic(path: &Path, plan: &ParsedPlan) -> Result<(), ArtifactError> {
    let rendered = canonical(path, plan)?;
    persist_replace(path, rendered.as_bytes())
}

/// Atomically replace a rendered text artifact (Markdown/HTML) at the path.
pub fn save_text_atomic(path: &Path, rendered: &str) -> Result<(), ArtifactError> {
    persist_replace(path, rendered.as_bytes())
}

/// Atomically replace a file with raw bytes (a stored transcript).
pub fn save_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), ArtifactError> {
    persist_replace(path, bytes)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    fn minimal_plan_json() -> &'static str {
        include_str!("../tests/fixtures/evidence-first-minimal-plan.json")
    }

    #[test]
    fn test_load_reads_and_parses_a_plan_artifact() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("plan.json");
        std::fs::write(&path, minimal_plan_json()).expect("write plan");
        let plan = load(&path).expect("valid plan loads");
        assert_eq!(plan.slug().as_str(), "add-store-metrics");
    }

    #[test]
    fn test_unknown_top_level_field_is_rejected_with_a_pointer() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("plan.json");
        let body = minimal_plan_json().replace(
            "\"slug\": \"add-store-metrics\",",
            "\"slug\": \"add-store-metrics\", \"notes\": \"hi\",",
        );
        std::fs::write(&path, body).expect("write plan");
        let error = load(&path).expect_err("unknown fields are rejected");
        match error {
            ArtifactError::Parse { source, .. } => {
                assert!(source.message().contains("unknown field `notes`"));
            }
            other => panic!("expected a parse error, got {other:?}"),
        }
    }

    #[test]
    fn test_create_new_then_save_atomic_round_trips_canonically() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("nested").join("plan.json");
        let plan = load_from_str(minimal_plan_json());

        create_new(&path, &plan).expect("create writes a new artifact");
        assert!(matches!(
            create_new(&path, &plan),
            Err(ArtifactError::Exists { .. })
        ));

        let reloaded = load(&path).expect("reload");
        save_atomic(&path, &reloaded).expect("save replaces atomically");
        let twice = load(&path).expect("reload again");
        assert_eq!(
            twice.to_canonical_json().unwrap(),
            plan.to_canonical_json().unwrap()
        );
    }

    fn load_from_str(json: &str) -> ParsedPlan {
        ParsedPlan::parse(json.as_bytes()).expect("valid plan")
    }
}
