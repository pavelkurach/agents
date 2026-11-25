//! A finding's `proof`: a new test committed on a local proofs branch that fails at head.

use std::collections::BTreeMap;
use std::fmt;
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

/// The reproduction paragraph of a proven finding, as report.md and the comments show it.
/// Output is indented as a code block, so no line of it reads as a heading or a `Fix:` line.
pub fn render(proof: &Proof) -> String {
    let mut out = format!(
        "Reproduced by `{}` — `{}`:\n\n{}",
        proof.test,
        proof.cmd.as_str().trim(),
        indent(&proof.head)
    );
    if let AtBase::Output(base) = &proof.base {
        out.push_str("\n\nAt base:\n\n");
        out.push_str(&indent(base));
    }
    out
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
