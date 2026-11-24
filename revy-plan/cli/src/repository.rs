use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::primitives::{NonEmptyText, PrimitiveError, RepoKey, Sha256Digest};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DirtySnapshot {
    pub changed_paths: u32,
    pub digest: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    pub root: NonEmptyText,
    pub name: RepoKey,
    pub revision: NonEmptyText,
    pub dirty: DirtySnapshot,
    pub captured_at: NonEmptyText,
}

impl Repository {
    pub fn matches(&self, other: &Self) -> bool {
        self.revision == other.revision && self.dirty == other.dirty
    }

    /// Whether an evidence-borne [`RepositoryIdentity`] names this repository's
    /// exact state — same revision and same dirty digest. The identity omits the
    /// changed-path count, so only revision and digest are compared. Used to gate
    /// that a verified plan's evidence was observed against the plan's own state.
    pub fn identity_matches(&self, identity: &crate::capability::RepositoryIdentity) -> bool {
        self.revision == identity.revision && self.dirty.digest == identity.dirty_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRepository {
    pub root: NonEmptyText,
    pub name: RepoKey,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("`{command}` failed in {root}: {detail}")]
    Git {
        command: String,
        root: String,
        detail: String,
    },
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, RepositoryError> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| RepositoryError::Git {
            command: format!("git {}", args.join(" ")),
            root: root.display().to_string(),
            detail: error.to_string(),
        })?;

    if output.status.success() {
        return Ok(strip_record_terminator(&String::from_utf8_lossy(&output.stdout)).to_owned());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let detail = if detail.is_empty() {
        output.status.to_string()
    } else {
        detail
    };

    Err(RepositoryError::Git {
        command: format!("git {}", args.join(" ")),
        root: root.display().to_string(),
        detail,
    })
}

fn strip_record_terminator(raw: &str) -> &str {
    raw.strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .unwrap_or(raw)
}

pub fn resolve(target: &Path) -> Result<ResolvedRepository, RepositoryError> {
    let root = git_output(target, &["rev-parse", "--show-toplevel"])?;
    Ok(ResolvedRepository {
        root: NonEmptyText::parse(&root)?,
        // Total: every checkout has a key, whatever its directory is called.
        name: RepoKey::for_root(Path::new(&root)),
    })
}

pub fn capture(target: &Path, now: &str) -> Result<Repository, RepositoryError> {
    let resolved = resolve(target)?;
    let revision = git_output(target, &["rev-parse", "HEAD"])?;
    let porcelain = git_output(target, &["status", "--porcelain=v1"])?;
    let line_count = porcelain
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let changed_paths = line_count.try_into().map_err(|_| RepositoryError::Git {
        command: "git status --porcelain=v1".to_owned(),
        root: resolved.root.as_str().to_owned(),
        detail: format!("reported more than u32::MAX changed paths ({line_count})"),
    })?;
    Ok(Repository {
        root: resolved.root,
        name: resolved.name,
        revision: NonEmptyText::parse(&revision)?,
        dirty: DirtySnapshot {
            changed_paths,
            digest: Sha256Digest::hash(porcelain.as_bytes()),
        },
        captured_at: NonEmptyText::parse(now)?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} failed");
    }

    #[test]
    fn test_resolving_a_checkout_whose_directory_name_has_an_underscore_succeeds() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let root = dir.path().join("agents_md");
        std::fs::create_dir(&root).expect("mkdir");
        git(&root, &["init", "-q"]);

        let resolved = resolve(&root).expect("an underscore is an ordinary directory name");

        assert!(
            resolved.name.as_str().starts_with("agents_md-"),
            "{}",
            resolved.name
        );
    }
}
