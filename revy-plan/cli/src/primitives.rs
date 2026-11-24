use std::borrow::Cow;
use std::path::{Component, Path};

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrimitiveError {
    #[error("value must not be empty")]
    Empty,
    #[error("path must be repository-relative, not absolute")]
    AbsolutePath,
    #[error("path must not escape the repository root")]
    PathEscape,
    #[error("path must be canonical: no `.` components, no trailing or repeated separators")]
    NonCanonicalPath,
    #[error("text must contain at least one non-whitespace character")]
    BlankText,
    #[error("slug must match [a-z0-9]+(-[a-z0-9]+)*")]
    MalformedSlug,
    #[error(
        "repository key must be one safe path segment: no `/` or `\\`, no control characters, and neither `.` nor `..`"
    )]
    MalformedRepoKey,
    #[error("collection must contain at least one entry")]
    EmptyCollection,
    #[error("SHA-256 digest must be exactly 64 lowercase hexadecimal characters")]
    MalformedSha256,
    #[error("program name must be a single token without whitespace")]
    MalformedProgramName,
}

macro_rules! parsed_string {
    ($name:ident, $parse:item) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(d)?;
                // Name the offending value: a bare "slug must match ..." on a
                // deserialize failure hides which field and which value was wrong.
                Self::parse(&raw)
                    .map_err(|error| serde::de::Error::custom(format!("{error}: {raw:?}")))
            }
        }

        impl $name {
            $parse
        }
    };
}

parsed_string!(
    RepoRelPath,
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        if raw.is_empty() {
            return Err(PrimitiveError::Empty);
        }
        let path = Path::new(raw);
        for component in path.components() {
            match component {
                Component::ParentDir => return Err(PrimitiveError::PathEscape),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(PrimitiveError::AbsolutePath);
                }
                Component::CurDir => return Err(PrimitiveError::NonCanonicalPath),
                Component::Normal(_) => {}
            }
        }
        if raw.ends_with('/') || raw.contains("//") {
            return Err(PrimitiveError::NonCanonicalPath);
        }
        Ok(Self(raw.to_owned()))
    }
);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        if raw.len() != 64
            || !raw
                .chars()
                .all(|character| matches!(character, '0'..='9' | 'a'..='f'))
        {
            return Err(PrimitiveError::MalformedSha256);
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn hash(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

parsed_string!(
    NonEmptyText,
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        if raw.trim().is_empty() {
            return Err(PrimitiveError::BlankText);
        }
        Ok(Self(raw.to_owned()))
    }
);

impl NonEmptyText {
    pub(crate) fn from_static(raw: &'static str) -> Self {
        Self(raw.to_owned())
    }
}

parsed_string!(
    Slug,
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        if raw.is_empty() {
            return Err(PrimitiveError::Empty);
        }
        let shaped = raw.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        });
        if !shaped {
            return Err(PrimitiveError::MalformedSlug);
        }
        Ok(Self(raw.to_owned()))
    }
);

/// Kept short enough that the digest-suffixed key stays readable and well
/// under any filesystem's filename limit.
const REPO_KEY_NAME_CHARS: usize = 64;
/// A prefix of the root-path digest, appended so two checkouts of the same
/// directory name in different places do not collide on one key.
const REPO_KEY_DIGEST_CHARS: usize = 12;
/// Used when a root path has no usable final component (`/`, or a name that
/// folds away entirely); the digest suffix still makes the key unique.
const REPO_KEY_FALLBACK_NAME: &str = "repo";

parsed_string!(
    RepoKey,
    /// One safe path segment naming a checkout directory. A slug grammar was
    /// too strict for a real directory name — it made any repository whose
    /// folder holds an underscore unplannable — so a key is any single segment
    /// that is not `.`/`..` and carries no separator or control character.
    /// [`RepoKey::for_root`] is the total constructor; `parse` serves the
    /// boundary of reading a key back out of a stored plan.
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        if raw.is_empty() || raw == "." || raw == ".." {
            return Err(PrimitiveError::MalformedRepoKey);
        }
        if raw.chars().any(|c| c == '/' || c == '\\' || c.is_control()) {
            return Err(PrimitiveError::MalformedRepoKey);
        }
        Ok(Self(raw.to_owned()))
    }
);

impl RepoKey {
    /// The key for the checkout rooted at `root` — total, and injective in
    /// `root` up to the digest prefix. The readable half is the directory name
    /// with anything outside `[A-Za-z0-9._-]` folded to `-`.
    pub fn for_root(root: &Path) -> Self {
        let name = root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .map(readable_name)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| REPO_KEY_FALLBACK_NAME.to_owned());
        let digest: String = Sha256Digest::hash(root.as_os_str().as_encoded_bytes())
            .as_str()
            .chars()
            .take(REPO_KEY_DIGEST_CHARS)
            .collect();
        Self(format!("{name}-{digest}"))
    }
}

fn readable_name(raw: &str) -> String {
    raw.chars()
        .take(REPO_KEY_NAME_CHARS)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['.', '-'])
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct OneOrMore<T>(Vec<T>);

// `OneOrMore<T>` is inlined into every schema that carries one, so schemars
// never generates a `OneOrMoreN` definition per element type (design §10): the
// wire shape is just a `Vec<T>` with `minItems: 1`.
impl<T: JsonSchema> JsonSchema for OneOrMore<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(format!("OneOrMore_of_{}", T::schema_name()))
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(format!("OneOrMore<{}>", T::schema_id()))
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = <Vec<T> as JsonSchema>::json_schema(generator);
        schema.insert("minItems".to_owned(), serde_json::json!(1));
        schema
    }
}

impl<T> OneOrMore<T> {
    pub fn parse(values: Vec<T>) -> Result<Self, PrimitiveError> {
        if values.is_empty() {
            return Err(PrimitiveError::EmptyCollection);
        }
        Ok(Self(values))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.0.iter()
    }

    pub(crate) fn from_one(value: T) -> Self {
        Self(vec![value])
    }

    /// Map each element into a new [`OneOrMore`], preserving non-emptiness: the
    /// result is guaranteed non-empty because the source is.
    pub fn map<U, F: Fn(&T) -> U>(&self, f: F) -> OneOrMore<U> {
        OneOrMore(self.0.iter().map(f).collect())
    }

    pub(crate) fn from_first(first: T, mut rest: Vec<T>) -> Self {
        rest.insert(0, first);
        Self(rest)
    }
}

impl<'a, T> IntoIterator for &'a OneOrMore<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for OneOrMore<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = Vec::<T>::deserialize(d)?;
        Self::parse(values).map_err(serde::de::Error::custom)
    }
}

/// The `argv[0]` of an executable command: a single non-empty token with no
/// whitespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ProgramName(NonEmptyText);

impl ProgramName {
    pub fn parse(raw: &str) -> Result<Self, PrimitiveError> {
        let text = NonEmptyText::parse(raw)?;
        if raw.chars().any(char::is_whitespace) {
            return Err(PrimitiveError::MalformedProgramName);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ProgramName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for ProgramName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// A single argv entry a command receives. Empty entries are valid process
/// input; interior NUL is not, since a NUL cannot survive the process boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Argument(String);

impl Argument {
    /// Build an argument from any string a process could receive. Empty entries
    /// are valid; deserialization additionally rejects interior NUL.
    pub const fn new(raw: String) -> Self {
        Self(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Argument {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.contains('\0') {
            return Err(serde::de::Error::custom(
                "argument must not contain a NUL byte",
            ));
        }
        Ok(Self(raw))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub path: RepoRelPath,
    pub line: u32,
    pub column: u32,
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

    #[test]
    fn test_repo_rel_path_accepts_a_nested_relative_path() {
        let path = RepoRelPath::parse("crates/revy-plan/src/lib.rs").expect("valid path");
        assert_eq!(path.as_str(), "crates/revy-plan/src/lib.rs");
    }

    #[test]
    fn test_repo_rel_path_rejects_an_absolute_path() {
        assert_eq!(
            RepoRelPath::parse("/etc/passwd"),
            Err(PrimitiveError::AbsolutePath)
        );
    }

    #[test]
    fn test_repo_rel_path_rejects_a_parent_escape() {
        assert_eq!(
            RepoRelPath::parse("../secrets.txt"),
            Err(PrimitiveError::PathEscape)
        );
    }

    #[test]
    fn test_repo_rel_path_rejects_an_interior_parent_escape() {
        assert_eq!(
            RepoRelPath::parse("src/../../out"),
            Err(PrimitiveError::PathEscape)
        );
    }

    #[test]
    fn test_repo_rel_path_rejects_a_current_directory_component() {
        assert_eq!(
            RepoRelPath::parse("./src/lib.rs"),
            Err(PrimitiveError::NonCanonicalPath)
        );
    }

    #[test]
    fn test_repo_rel_path_rejects_an_empty_string() {
        assert_eq!(RepoRelPath::parse(""), Err(PrimitiveError::Empty));
    }

    #[test]
    fn test_non_empty_text_rejects_whitespace_only() {
        assert_eq!(NonEmptyText::parse("   \n"), Err(PrimitiveError::BlankText));
    }

    #[test]
    fn test_slug_rejects_upper_case() {
        assert_eq!(Slug::parse("Revy-Plan"), Err(PrimitiveError::MalformedSlug));
    }

    #[test]
    fn test_slug_rejects_leading_hyphen() {
        assert_eq!(Slug::parse("-plan"), Err(PrimitiveError::MalformedSlug));
    }

    #[test]
    fn test_slug_accepts_kebab_case_with_digits() {
        assert_eq!(
            Slug::parse("revy-plan-2").expect("valid slug").as_str(),
            "revy-plan-2"
        );
    }

    #[test]
    fn test_repo_key_for_root_keeps_an_underscore_name_and_appends_a_digest() {
        let key = RepoKey::for_root(Path::new("/Users/dev/agents_md"));
        assert!(key.as_str().starts_with("agents_md-"), "{}", key.as_str());
    }

    #[test]
    fn test_repo_key_for_root_disambiguates_same_named_checkouts_in_different_places() {
        let first = RepoKey::for_root(Path::new("/work/cli"));
        let second = RepoKey::for_root(Path::new("/fork/cli"));
        assert_ne!(first, second);
    }

    #[test]
    fn test_repo_key_parse_accepts_a_stored_single_segment() {
        assert_eq!(
            RepoKey::parse("codecrafters-redis-rust")
                .expect("valid key")
                .as_str(),
            "codecrafters-redis-rust"
        );
    }

    #[test]
    fn test_repo_key_parse_rejects_a_path_separator() {
        assert_eq!(
            RepoKey::parse("runs/etc"),
            Err(PrimitiveError::MalformedRepoKey)
        );
    }

    #[test]
    fn test_repo_key_parse_rejects_a_parent_segment() {
        assert_eq!(RepoKey::parse(".."), Err(PrimitiveError::MalformedRepoKey));
    }

    #[test]
    fn test_one_or_more_rejects_an_empty_vector() {
        assert_eq!(
            OneOrMore::<u32>::parse(vec![]),
            Err(PrimitiveError::EmptyCollection)
        );
    }

    #[test]
    fn test_one_or_more_keeps_element_order() {
        let values = OneOrMore::parse(vec![3_u32, 1, 2]).expect("non-empty");
        assert_eq!(values.as_slice(), &[3, 1, 2]);
    }

    #[test]
    fn test_program_name_rejects_interior_whitespace() {
        ProgramName::parse("cargo build").expect_err("a program name is a single argv token");
    }

    #[test]
    fn test_program_name_accepts_a_single_token() {
        assert_eq!(
            ProgramName::parse("rust-analyzer")
                .expect("valid program name")
                .as_str(),
            "rust-analyzer"
        );
    }

    #[test]
    fn test_argument_rejects_an_interior_nul() {
        serde_json::from_str::<Argument>("\"a\\u0000b\"").expect_err("interior NUL is invalid");
    }

    #[test]
    fn test_argument_accepts_an_empty_string() {
        let argument: Argument = serde_json::from_str(r#""""#).expect("empty argument is valid");
        assert_eq!(argument.as_str(), "");
    }

    #[test]
    fn test_location_rejects_a_boolean_line() {
        let mut deserializer =
            serde_json::Deserializer::from_str(r#"{"path":"src/lib.rs","line":true,"column":0}"#);
        let error = serde_path_to_error::deserialize::<_, Location>(&mut deserializer)
            .expect_err("boolean is not an integer");
        assert_eq!(error.path().to_string(), "line");
    }

    #[test]
    fn test_location_rejects_a_negative_line() {
        serde_json::from_str::<Location>(r#"{"path":"src/lib.rs","line":-1,"column":0}"#)
            .expect_err("negative line is invalid");
    }

    #[test]
    fn test_location_rejects_an_unknown_field() {
        serde_json::from_str::<Location>(
            r#"{"path":"src/lib.rs","line":6,"column":0,"snippet":"fn add"}"#,
        )
        .expect_err("unknown fields are rejected");
    }

    #[test]
    fn test_location_round_trips_through_json() {
        let json = r#"{"path":"src/lib.rs","line":6,"column":4}"#;
        let location: Location = serde_json::from_str(json).expect("valid location");
        assert_eq!(serde_json::to_string(&location).expect("serializes"), json);
    }
}
