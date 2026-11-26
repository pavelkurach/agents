use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::primitives::{NonEmptyText, PrimitiveError, RepoAlias, RepoKey, Sha256Digest};

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

    /// The evidence-borne identity of this repository state.
    pub fn identity(&self) -> crate::capability::RepositoryIdentity {
        crate::capability::RepositoryIdentity {
            revision: self.revision.clone(),
            dirty_digest: self.dirty.digest.clone(),
        }
    }
}

/// One of the plan's repositories, as a citation resolves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repo<'a> {
    pub alias: &'a RepoAlias,
    pub repository: &'a Repository,
    pub primary: bool,
}

/// Why a cited repository alias does not resolve.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CitationError {
    #[error(
        "repository alias `{alias}` is not declared; declared aliases: {}; declare it with `revy-plan init --repo {alias}=PATH` or `revy-plan reopen exploration --repo {alias}=PATH`",
        backticked(declared)
    )]
    UnknownAlias {
        alias: RepoAlias,
        declared: Vec<RepoAlias>,
    },
    #[error(
        "`{alias}:` names the primary repository, which is cited unprefixed; drop the `{alias}:` prefix"
    )]
    PrimaryPrefixed { alias: RepoAlias },
}

fn backticked(aliases: &[RepoAlias]) -> String {
    aliases
        .iter()
        .map(|alias| format!("`{alias}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Why a set of repositories cannot be declared together.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeclarationError {
    #[error("repository alias `{alias}` is declared twice")]
    DuplicateAlias { alias: RepoAlias },
    #[error(
        "repository alias `{alias}` is the primary repository's alias; choose another alias (or rename the primary with `--alias`)"
    )]
    PrimaryAlias { alias: RepoAlias },
    #[error("repositories `{first}` and `{second}` are the same checkout `{root}`")]
    SameCheckout {
        first: RepoAlias,
        second: RepoAlias,
        root: NonEmptyText,
    },
}

/// The plan's repositories: the primary one (which keys the plan home and is
/// cited unprefixed) and every other declared one, by alias. Aliases and
/// checkouts are distinct by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repositories {
    primary_alias: RepoAlias,
    primary: Repository,
    others: BTreeMap<RepoAlias, Repository>,
}

impl Repositories {
    pub fn new(
        primary_alias: RepoAlias,
        primary: Repository,
        others: impl IntoIterator<Item = (RepoAlias, Repository)>,
    ) -> Result<Self, DeclarationError> {
        let mut declared = Self {
            primary_alias,
            primary,
            others: BTreeMap::new(),
        };
        for (alias, repository) in others {
            declared = declared.with(alias, repository)?;
        }
        Ok(declared)
    }

    /// These repositories plus one more.
    pub fn with(
        mut self,
        alias: RepoAlias,
        repository: Repository,
    ) -> Result<Self, DeclarationError> {
        if alias == self.primary_alias {
            return Err(DeclarationError::PrimaryAlias { alias });
        }
        if self.others.contains_key(&alias) {
            return Err(DeclarationError::DuplicateAlias { alias });
        }
        if let Some(existing) = self
            .iter()
            .find(|repo| repo.repository.root == repository.root)
        {
            return Err(DeclarationError::SameCheckout {
                first: existing.alias.clone(),
                second: alias,
                root: repository.root,
            });
        }
        self.others.insert(alias, repository);
        Ok(self)
    }

    /// The same repositories under the same aliases, each replaced by a fresh
    /// capture of its checkout.
    pub fn recaptured<E>(
        &self,
        mut capture: impl FnMut(&Repository) -> Result<Repository, E>,
    ) -> Result<Self, E> {
        Ok(Self {
            primary_alias: self.primary_alias.clone(),
            primary: capture(&self.primary)?,
            others: self
                .others
                .iter()
                .map(|(alias, repository)| Ok((alias.clone(), capture(repository)?)))
                .collect::<Result<_, E>>()?,
        })
    }

    pub const fn primary_alias(&self) -> &RepoAlias {
        &self.primary_alias
    }

    pub const fn primary(&self) -> &Repository {
        &self.primary
    }

    /// The declared repositories other than the primary.
    pub const fn others(&self) -> &BTreeMap<RepoAlias, Repository> {
        &self.others
    }

    pub const fn primary_repo(&self) -> Repo<'_> {
        Repo {
            alias: &self.primary_alias,
            repository: &self.primary,
            primary: true,
        }
    }

    /// Every repository, the primary first.
    pub fn iter(&self) -> impl Iterator<Item = Repo<'_>> {
        std::iter::once(self.primary_repo()).chain(self.others.iter().map(|(alias, repository)| {
            Repo {
                alias,
                repository,
                primary: false,
            }
        }))
    }

    /// Every alias, the primary first.
    pub fn aliases(&self) -> Vec<RepoAlias> {
        self.iter().map(|repo| repo.alias.clone()).collect()
    }

    /// The repository a citation names: `None` is the primary.
    pub fn resolve(&self, alias: Option<&RepoAlias>) -> Result<Repo<'_>, CitationError> {
        let Some(alias) = alias else {
            return Ok(self.primary_repo());
        };
        if *alias == self.primary_alias {
            return Err(CitationError::PrimaryPrefixed {
                alias: alias.clone(),
            });
        }
        self.others
            .get_key_value(alias)
            .map(|(alias, repository)| Repo {
                alias,
                repository,
                primary: false,
            })
            .ok_or_else(|| CitationError::UnknownAlias {
                alias: alias.clone(),
                declared: self.aliases(),
            })
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

    fn repository(root: &str) -> Repository {
        Repository {
            root: NonEmptyText::parse(root).unwrap(),
            name: RepoKey::for_root(Path::new(root)),
            revision: NonEmptyText::parse("rev").unwrap(),
            dirty: DirtySnapshot {
                changed_paths: 0,
                digest: Sha256Digest::hash(b""),
            },
            captured_at: NonEmptyText::parse("2026-01-01T00:00:00Z").unwrap(),
        }
    }

    fn alias(raw: &str) -> RepoAlias {
        RepoAlias::parse(raw).unwrap()
    }

    fn dashboard_and_vortex() -> Repositories {
        Repositories::new(
            alias("dashboard"),
            repository("/work/dashboard"),
            [(alias("vortex"), repository("/work/vortex"))],
        )
        .unwrap()
    }

    #[test]
    fn test_an_unprefixed_citation_resolves_to_the_primary() {
        let repositories = dashboard_and_vortex();
        assert_eq!(
            repositories.resolve(None).unwrap().alias.as_str(),
            "dashboard"
        );
    }

    #[test]
    fn test_a_declared_alias_resolves_to_its_repository() {
        let repositories = dashboard_and_vortex();
        let repo = repositories.resolve(Some(&alias("vortex"))).unwrap();
        assert_eq!(repo.repository.root.as_str(), "/work/vortex");
    }

    #[test]
    fn test_an_undeclared_alias_lists_every_declared_alias() {
        let repositories = dashboard_and_vortex();
        assert_eq!(
            repositories.resolve(Some(&alias("mistral"))),
            Err(CitationError::UnknownAlias {
                alias: alias("mistral"),
                declared: vec![alias("dashboard"), alias("vortex")],
            })
        );
    }

    #[test]
    fn test_the_primary_alias_as_a_prefix_is_rejected() {
        let repositories = dashboard_and_vortex();
        assert_eq!(
            repositories.resolve(Some(&alias("dashboard"))),
            Err(CitationError::PrimaryPrefixed {
                alias: alias("dashboard")
            })
        );
    }

    #[test]
    fn test_declaring_an_alias_twice_is_rejected() {
        let error = dashboard_and_vortex()
            .with(alias("vortex"), repository("/work/other"))
            .unwrap_err();
        assert_eq!(
            error,
            DeclarationError::DuplicateAlias {
                alias: alias("vortex")
            }
        );
    }

    #[test]
    fn test_declaring_the_primary_alias_again_is_rejected() {
        let error = dashboard_and_vortex()
            .with(alias("dashboard"), repository("/work/other"))
            .unwrap_err();
        assert_eq!(
            error,
            DeclarationError::PrimaryAlias {
                alias: alias("dashboard")
            }
        );
    }

    #[test]
    fn test_declaring_the_same_checkout_under_two_aliases_is_rejected() {
        let error = dashboard_and_vortex()
            .with(alias("engine"), repository("/work/vortex"))
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "repositories `vortex` and `engine` are the same checkout `/work/vortex`"
        );
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
