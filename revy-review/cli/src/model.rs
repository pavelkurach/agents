//! Domain primitives shared by every format: sites, ids, severities, state.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;

/// One problem found in an input, located as `path:line` or `file: json.path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub at: String,
    pub message: String,
}

impl Diagnostic {
    pub fn new(at: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            at: at.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

/// Deserializes from a JSON string through `FromStr`, so JSON and report.md share one parser.
macro_rules! deserialize_from_str {
    ($($ty:ty),+) => {$(
        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                <String as serde::Deserialize>::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    )+};
}
pub(crate) use deserialize_from_str;

/// A numbered id: `<prefix><k>`, or `<reviewer><sep><k>` for ids a reviewer owns.
macro_rules! numbered_id {
    ($(#[$doc:meta])* $name:ident = $prefix:literal k, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU32);

        impl FromStr for $name {
            type Err = String;
            fn from_str(raw: &str) -> Result<Self, String> {
                raw.strip_prefix($prefix)
                    .and_then(|k| k.parse().ok())
                    .map(Self)
                    .ok_or_else(|| format!("`{raw}` is not a {} `{}<k>`", $what, $prefix))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }

        deserialize_from_str!($name);
    };
    ($(#[$doc:meta])* $name:ident = n $sep:literal k, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name {
            pub reviewer: NonZeroU32,
            k: NonZeroU32,
        }

        impl FromStr for $name {
            type Err = String;
            fn from_str(raw: &str) -> Result<Self, String> {
                raw.split_once($sep)
                    .and_then(|(n, k)| Some(Self { reviewer: n.parse().ok()?, k: k.parse().ok()? }))
                    .ok_or_else(|| format!("`{raw}` is not a {} `<n>{}<k>`", $what, $sep))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}{}", self.reviewer, $sep, self.k)
            }
        }

        deserialize_from_str!($name);
    };
}

numbered_id!(FindingId = n "." k, "finding id");
numbered_id!(SuspicionId = n ".s" k, "suspicion id");
numbered_id!(VerifiedId = "F" k, "verified finding id");
numbered_id!(CriterionId = "C" k, "criterion id");
numbered_id!(
    /// One scope-creep entry per cause, however many sites it spans.
    ScopeCreepId = "S" k,
    "scope-creep id"
);

/// Repository-relative file path: non-empty, no leading `/`, no `..` segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

impl RepoPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RepoPath {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        if raw.is_empty() || raw.starts_with('/') || raw.split('/').any(|part| part == "..") {
            return Err(format!("`{raw}` is not a repository-relative path"));
        }
        Ok(Self(raw.to_owned()))
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `path:line`, line 1-based at head.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Site {
    pub path: RepoPath,
    pub line: NonZeroU32,
}

impl FromStr for Site {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        let malformed =
            || format!("site `{raw}` is not `path:line` (repo-relative path, line from 1)");
        let (path, line) = raw.rsplit_once(':').ok_or_else(malformed)?;
        Ok(Self {
            path: path.parse().map_err(|_| malformed())?,
            line: line.parse().map_err(|_| malformed())?,
        })
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path, self.line)
    }
}

/// At least one site; the first is where the quote is and where a comment posts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Vec<Site>")]
pub struct Sites {
    first: Site,
    rest: Vec<Site>,
}

impl Sites {
    pub const fn first(&self) -> &Site {
        &self.first
    }

    pub fn rest(&self) -> &[Site] {
        &self.rest
    }

    pub fn all(&self) -> impl Iterator<Item = &Site> {
        std::iter::once(&self.first).chain(&self.rest)
    }
}

impl TryFrom<Vec<Site>> for Sites {
    type Error = &'static str;
    fn try_from(sites: Vec<Site>) -> Result<Self, Self::Error> {
        let mut sites = sites.into_iter();
        let first = sites
            .next()
            .ok_or("sites must name at least one `path:line`")?;
        Ok(Self {
            first,
            rest: sites.collect(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Blocking,
    NonBlocking,
    Question,
    Suggestion,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::NonBlocking => "non-blocking",
            Self::Question => "question",
            Self::Suggestion => "suggestion",
        }
    }
}

impl FromStr for Severity {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        [
            Self::Blocking,
            Self::NonBlocking,
            Self::Question,
            Self::Suggestion,
        ]
        .into_iter()
        .find(|severity| severity.as_str() == raw)
        .ok_or_else(|| {
            format!("severity `{raw}` is not one of blocking, non-blocking, question, suggestion")
        })
    }
}

/// Text that is not blank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Text {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        if raw.trim().is_empty() {
            Err("text must not be blank".to_owned())
        } else {
            Ok(Self(raw.to_owned()))
        }
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A commit object name: 4–64 hex digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha(String);

impl Sha {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Sha {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        if (4..=64).contains(&raw.len()) && raw.chars().all(|c| c.is_ascii_hexdigit()) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(format!("`{raw}` is not a commit sha"))
        }
    }
}

deserialize_from_str!(Site, Text, Sha);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "PathBuf")]
pub struct AbsolutePath(PathBuf);

impl AbsolutePath {
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl TryFrom<PathBuf> for AbsolutePath {
    type Error = String;
    fn try_from(path: PathBuf) -> Result<Self, String> {
        if path.is_absolute() {
            Ok(Self(path))
        } else {
            Err(format!("`{}` is not an absolute path", path.display()))
        }
    }
}

/// The coordinator's per-review decision for one specialist.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// `"dispatched"`
    Dispatched,
    /// `{"skipped": "<reason>"}`
    Skipped(Text),
}

/// The fields of `state.json` the helper reads; `mode` and `round` steer only the skill.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct State {
    pub repo: AbsolutePath,
    pub target: Text,
    pub base: Sha,
    pub head: Sha,
    /// This skill's directory: its `rules/` and `reviewers/` (specialists) load first.
    pub skill: AbsolutePath,
    /// Specialist name → decision; absent until the coordinator decides, before step 1.
    pub specialists: Option<BTreeMap<String, Decision>>,
}

impl State {
    /// `(name, reason)` of every skipped specialist, in name order.
    pub fn skipped(&self) -> Vec<(&str, &Text)> {
        self.specialists
            .iter()
            .flatten()
            .filter_map(|(name, decision)| match decision {
                Decision::Skipped(reason) => Some((name.as_str(), reason)),
                Decision::Dispatched => None,
            })
            .collect()
    }
}

/// Parses JSON into `T`, naming the failing field path on error.
pub fn parse_json<T: serde::de::DeserializeOwned>(file: &str, text: &str) -> Result<T, Diagnostic> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
        let path = error.path().to_string();
        let at = if path == "." {
            file.to_owned()
        } else {
            format!("{file}: {path}")
        };
        Diagnostic::new(at, error.into_inner().to_string())
    })
}

/// `(index, first index)` for every item whose key already appeared earlier.
pub fn repeats<T, K: Ord>(items: &[T], key: impl Fn(&T) -> K) -> Vec<(usize, usize)> {
    let mut first: BTreeMap<K, usize> = BTreeMap::new();
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| match first.entry(key(item)) {
            Entry::Occupied(earlier) => Some((index, *earlier.get())),
            Entry::Vacant(slot) => {
                slot.insert(index);
                None
            }
        })
        .collect()
}
