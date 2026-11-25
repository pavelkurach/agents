//! A finding's `proof`: a new test committed on a local proofs branch that fails at head.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::str::FromStr;

use serde::Deserialize;

use crate::model::{Diagnostic, RepoPath, Sha, Text, deserialize_from_str};

/// How many characters of head name a review's proofs branches.
const SHORT_SHA: usize = 7;

/// `<path>::<test name or line>`: the test file at the proof commit, and the test in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRef {
    pub path: RepoPath,
    pub name: Text,
}

impl FromStr for TestRef {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        let malformed =
            || format!("test `{raw}` is not `<repo-relative path>::<test name or line>`");
        let (path, name) = raw.split_once("::").ok_or_else(malformed)?;
        Ok(Self {
            path: path.parse().map_err(|_| malformed())?,
            name: name.parse().map_err(|_| malformed())?,
        })
    }
}

impl fmt::Display for TestRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.path, self.name)
    }
}

/// The proof test's result at base: its output, or `"n/a"` when it cannot run there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtBase {
    NotApplicable,
    Output(Text),
}

impl FromStr for AtBase {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        if raw.trim() == "n/a" {
            Ok(Self::NotApplicable)
        } else {
            raw.parse()
                .map(Self::Output)
                .map_err(|_| "base is the test's output at base, or \"n/a\"".to_owned())
        }
    }
}

deserialize_from_str!(TestRef, AtBase);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    pub test: TestRef,
    /// The commit on a proofs branch that adds the test.
    pub commit: Sha,
    pub cmd: Text,
    /// The failing output excerpt at head.
    pub head: Text,
    pub base: AtBase,
}

/// The branch the verifier merges the kept findings' proving commits into.
pub fn merged_branch(head: &Sha) -> String {
    let head = head.as_str();
    format!("revy/{}/proofs", head.get(..SHORT_SHA).unwrap_or(head))
}

/// What one proof asks of the repository: its commit, its test file there, and,
/// for a kept finding, the branch the commit must be on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Query {
    pub commit: Sha,
    pub path: RepoPath,
    pub branch: Option<String>,
}

/// What git holds for a query, each state implying the ones before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    NoCommit,
    NoFile,
    OffBranch,
    Present,
}

/// Git's answers for every proof a review directory cites; an unasked query reads as no commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofObjects(BTreeMap<Query, Found>);

impl ProofObjects {
    pub const fn new(found: BTreeMap<Query, Found>) -> Self {
        Self(found)
    }

    fn get(&self, query: &Query) -> Found {
        self.0.get(query).copied().unwrap_or(Found::NoCommit)
    }
}

/// One cited proof: where it sits, and the branch its commit must be on.
#[derive(Debug, Clone)]
pub struct Cited<'a> {
    pub at: String,
    pub proof: &'a Proof,
    pub branch: Option<String>,
}

impl Cited<'_> {
    pub fn query(&self) -> Query {
        Query {
            commit: self.proof.commit.clone(),
            path: self.proof.test.path.clone(),
            branch: self.branch.clone(),
        }
    }
}

pub fn check(cited: &Cited<'_>, objects: &ProofObjects) -> Option<Diagnostic> {
    let commit = cited.proof.commit.as_str();
    let message = match objects.get(&cited.query()) {
        Found::Present => return None,
        Found::NoCommit => format!(
            "commit {commit} is not in the repository; commit the test on your proofs branch and cite that sha"
        ),
        Found::NoFile => format!(
            "test file {} does not exist at commit {commit}",
            cited.proof.test.path
        ),
        Found::OffBranch => format!(
            "commit {commit} is not on branch {}; cherry-pick each kept finding's proving commit there and cite the new sha",
            cited.branch.as_deref().unwrap_or_default()
        ),
    };
    Some(Diagnostic::new(&cited.at, message))
}

/// The proof test's source as a comment shows it, so the PR author can run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestSource {
    /// The test itself, cut from its file at the proof commit by the `::name` part.
    Test(String),
    /// The test file's diff in the proof commit, when the test cannot be found by name.
    Diff(String),
}

/// The test named by `test` in `file` (its text at the proof commit), else `diff` (the test
/// file's diff in that commit); `None` when neither holds anything.
pub fn source(test: &TestRef, file: Option<&str>, diff: &str) -> Option<TestSource> {
    let found = test_identifier(test.name.as_str())
        .and_then(|name| file.and_then(|file| find_test(file, name)));
    match found {
        Some(code) => Some(TestSource::Test(code)),
        None if diff.trim().is_empty() => None,
        None => Some(TestSource::Diff(diff.trim_end().to_owned())),
    }
}

const fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// The function name in a test name: its last `::` segment, before any parameters or
/// subtest (`test_x[1]`, `TestX/case`); `None` for a line number.
fn test_identifier(name: &str) -> Option<&str> {
    let last = name.rsplit("::").next().unwrap_or(name).trim();
    let end = last.find(|c| !is_identifier_char(c)).unwrap_or(last.len());
    let identifier = last.get(..end)?;
    identifier
        .chars()
        .next()
        .filter(|first| !first.is_ascii_digit())
        .map(|_| identifier)
}

fn contains_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(at, _)| {
        let before = line.get(..at).and_then(|text| text.chars().next_back());
        let after = line
            .get(at + word.len()..)
            .and_then(|text| text.chars().next());
        !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
    })
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The test defined at the first line naming `name`, with the attributes or decorators
/// directly above it: through its closing brace, or through its indented body when the
/// definition ends with `:`. `None` when the body has no end in the file.
fn find_test(file: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = file.lines().collect();
    let definition = lines.iter().position(|line| contains_word(line, name))?;
    let indent = indent_of(lines.get(definition)?);
    let attributes = lines
        .get(..definition)?
        .iter()
        .rev()
        .take_while(|line| {
            let trimmed = line.trim_start();
            indent_of(line) == indent && (trimmed.starts_with("#[") || trimmed.starts_with('@'))
        })
        .count();
    let start = definition - attributes;
    let end = body_end(&lines, definition, indent)?;
    Some(lines.get(start..=end)?.join("\n"))
}

/// The last line of the body that starts at `definition`.
fn body_end(lines: &[&str], definition: usize, indent: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut opened = false;
    for (index, line) in lines.iter().enumerate().skip(definition) {
        for c in line.chars() {
            match c {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth == 0 && line.trim_end().ends_with(':') {
            return indented_end(lines, index, indent);
        }
        if opened && depth == 0 {
            return Some(index);
        }
    }
    None
}

/// The last non-blank line indented deeper than `indent` after `header`.
fn indented_end(lines: &[&str], header: usize, indent: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(header + 1)
        .take_while(|(_, line)| line.trim().is_empty() || indent_of(line) > indent)
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, _)| index)
        .last()
}

/// The reproduction paragraph of a proven finding, as report.md and the comments show it.
///
/// The test's source sits in a fence report.md's parser skips, and its output is indented as
/// a code block, so no line of either reads as a heading or a `Fix:` line.
pub fn render(proof: &Proof, source: Option<&TestSource>) -> String {
    let mut out = format!(
        "Reproduced by `{}` — `{}`:\n\n",
        proof.test,
        proof.cmd.as_str().trim(),
    );
    if let Some(source) = source {
        let (info, code) = match source {
            TestSource::Test(code) => (
                proof
                    .test
                    .path
                    .as_str()
                    .rsplit('/')
                    .next()
                    .and_then(|file| file.rsplit_once('.'))
                    .map_or("", |(_, extension)| extension),
                code,
            ),
            TestSource::Diff(diff) => ("diff", diff),
        };
        let fence = "`".repeat(longest_backtick_run(code).max(2) + 1);
        let _ = write!(out, "{fence}{info}\n{code}\n{fence}\n\nAt head:\n\n");
    }
    out.push_str(&indent(&proof.head));
    if let AtBase::Output(base) = &proof.base {
        out.push_str("\n\nAt base:\n\n");
        out.push_str(&indent(base));
    }
    out
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

fn indent(text: &Text) -> String {
    text.as_str()
        .trim()
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("    {}", line.trim_end())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
