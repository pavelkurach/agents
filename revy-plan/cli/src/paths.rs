use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::primitives::{RepoKey, Slug};

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PathError {
    #[error("cannot resolve the plan home: set REVY_PLAN_HOME or HOME")]
    NoHome,
    #[error("plan home must be outside the target checkout")]
    InsideTarget,
}

pub fn plan_home() -> Result<PathBuf, PathError> {
    if let Some(explicit) = std::env::var_os("REVY_PLAN_HOME") {
        return Ok(PathBuf::from(explicit));
    }

    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".revy-plan"))
        .ok_or(PathError::NoHome)
}

/// The file naming the most recently initialized plan of any repository,
/// which plan-locating commands fall back to when run outside a git
/// repository.
pub fn current_plan_file() -> Result<PathBuf, PathError> {
    Ok(plan_home()?.join("current"))
}

/// The file naming the most recently initialized plan for one repository.
///
/// Plan-locating commands run inside that repository fall back to it. Kept beside, not under, the global `current` file so both can exist.
pub fn repo_current_plan_file(repo_name: &RepoKey) -> Result<PathBuf, PathError> {
    Ok(plan_home()?.join("current.d").join(repo_name.as_str()))
}

pub fn plan_dir(repo_name: &RepoKey, slug: &Slug) -> Result<PathBuf, PathError> {
    Ok(plan_home()?
        .join("plans")
        .join(repo_name.as_str())
        .join(slug.as_str()))
}

pub fn plan_path(repo_name: &RepoKey, slug: &Slug) -> Result<PathBuf, PathError> {
    Ok(plan_dir(repo_name, slug)?.join("plan.json"))
}

/// The external home for a plan's retained transcripts.
///
/// A `transcripts/` sibling of `plan.json`, outside the target checkout. Probe
/// and runtime-evidence transcript digests are verified against files under this
/// directory, never the immutable target.
pub fn transcripts_dir(repo_name: &RepoKey, slug: &Slug) -> Result<PathBuf, PathError> {
    Ok(plan_dir(repo_name, slug)?.join("transcripts"))
}

/// The shared, content-addressed transcript store: `transcripts/<sha256>` under
/// the plan home, consulted when a plan's own `transcripts/` lacks a file.
pub fn shared_transcripts_dir() -> Result<PathBuf, PathError> {
    Ok(plan_home()?.join("transcripts"))
}

fn invalid_path_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "path could not be resolved safely",
    )
}

fn absolute_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir().map(|current_dir| current_dir.join(path))
    }
}

fn canonicalize_existing_ancestor(path: &Path) -> Result<(PathBuf, PathBuf), std::io::Error> {
    let absolute = absolute_path(path)?;
    let mut ancestor = absolute.as_path();

    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(invalid_path_error)?;
    }

    let suffix = absolute
        .strip_prefix(ancestor)
        .map_err(|_| invalid_path_error())?
        .to_path_buf();
    let resolved = ancestor.canonicalize()?;
    Ok((resolved, suffix))
}

fn normalize_unresolved_suffix(
    mut resolved: PathBuf,
    suffix: &Path,
) -> Result<PathBuf, std::io::Error> {
    for component in suffix.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => resolved.push(part),
            std::path::Component::ParentDir => {
                if !resolved.pop() {
                    return Err(invalid_path_error());
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(invalid_path_error());
            }
        }
    }

    Ok(resolved)
}

fn resolve_destination_parent(path: &Path) -> Result<PathBuf, std::io::Error> {
    let (resolved_ancestor, unresolved_suffix) = canonicalize_existing_ancestor(path)?;
    normalize_unresolved_suffix(resolved_ancestor, &unresolved_suffix)
}

pub fn ensure_destination_parent_outside_target(
    destination_parent: &Path,
    target_root: &Path,
) -> Result<(), PathError> {
    let canonical_target = target_root
        .canonicalize()
        .map_err(|_| PathError::InsideTarget)?;

    let resolved_destination_parent =
        resolve_destination_parent(destination_parent).map_err(|_| PathError::InsideTarget)?;

    if resolved_destination_parent == canonical_target
        || resolved_destination_parent.starts_with(&canonical_target)
    {
        Err(PathError::InsideTarget)
    } else {
        Ok(())
    }
}
