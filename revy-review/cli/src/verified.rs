//! `verified.json`: the verifier's kept and dropped findings, each item accounted for once.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::diff::HeadFiles;
use crate::model::{
    CriterionId, Diagnostic, FindingId, RepoPath, ScopeCreepId, Severity, Site, Sites, Text,
    VerifiedId, deserialize_from_str, repeats,
};
use crate::problem::{CriterionVerdict, Problem};
use crate::proof::Proof;
use crate::review::{Review, check_finding};

/// What a kept or dropped finding came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    Reviewer(FindingId),
    Criterion(CriterionId),
    Scope(ScopeCreepId),
}

impl FromStr for Origin {
    type Err = String;
    fn from_str(raw: &str) -> Result<Self, String> {
        match raw.chars().next() {
            Some('C') => raw.parse().map(Self::Criterion),
            Some('S') => raw.parse().map(Self::Scope),
            _ => raw.parse().map(Self::Reviewer).map_err(|_| {
                format!("`{raw}` is not a reviewer finding `<n>.<k>`, a criterion `C<k>` or a scope-creep id `S<k>`")
            }),
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reviewer(id) => id.fmt(f),
            Self::Criterion(id) => id.fmt(f),
            Self::Scope(id) => id.fmt(f),
        }
    }
}

deserialize_from_str!(Origin);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedFinding {
    pub id: VerifiedId,
    pub title: Text,
    pub severity: Severity,
    pub sites: Sites,
    pub quote: String,
    pub breaks: Text,
    pub fix: Text,
    pub rule: Option<String>,
    pub proof: Option<Proof>,
    pub merged_from: Vec<Origin>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dropped {
    pub from: Origin,
    pub refuted_by: Site,
    pub why: Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verified {
    pub findings: Vec<VerifiedFinding>,
    pub dropped: Vec<Dropped>,
}

impl Verified {
    pub fn cited_paths(&self) -> impl Iterator<Item = &RepoPath> {
        let kept = self.findings.iter().flat_map(|finding| finding.sites.all());
        let dropped = self.dropped.iter().map(|dropped| &dropped.refuted_by);
        kept.chain(dropped).map(|site| &site.path)
    }
}

/// Every reviewer and investigator finding, and every gate item in `problem.json`,
/// each with how to name it.
pub fn expected(
    reviews: &[(String, Review)],
    problem: Option<&Problem>,
) -> BTreeMap<Origin, String> {
    let reviewer = reviews.iter().flat_map(|(file, review)| {
        review.findings.iter().map(move |finding| {
            (
                Origin::Reviewer(finding.id),
                format!("reviewer finding {} ({file})", finding.id),
            )
        })
    });
    let criteria = problem
        .into_iter()
        .flat_map(|problem| &problem.criteria)
        .filter(|c| {
            matches!(
                c.verdict,
                CriterionVerdict::NotMet | CriterionVerdict::Partly
            )
        })
        .map(|c| {
            (
                Origin::Criterion(c.id),
                format!("gate finding {} ({})", c.id, c.verdict.as_str()),
            )
        });
    let creep = problem
        .into_iter()
        .flat_map(|problem| &problem.scope_creep)
        .map(|creep| {
            (
                Origin::Scope(creep.id),
                format!("gate finding {} (scope creep)", creep.id),
            )
        });
    reviewer.chain(criteria).chain(creep).collect()
}

pub fn check(
    verified: &Verified,
    expected: &BTreeMap<Origin, String>,
    rules: &BTreeSet<String>,
    head: &HeadFiles,
) -> Vec<Diagnostic> {
    const FILE: &str = "verified.json";
    let mut errors = Vec::new();
    let mut uses: BTreeMap<Origin, Vec<String>> = BTreeMap::new();
    let mut reference = |at: String, origin: &Origin| {
        if expected.contains_key(origin) {
            uses.entry(*origin).or_default().push(at);
            None
        } else {
            Some(Diagnostic::new(format!("{FILE}: {at}"), unknown(origin)))
        }
    };
    for (index, finding) in verified.findings.iter().enumerate() {
        errors.extend(check_finding(
            &format!("{FILE}: findings[{index}]"),
            &finding.sites,
            &finding.quote,
            finding.rule.as_deref(),
            rules,
            head,
        ));
        for (origin_index, origin) in finding.merged_from.iter().enumerate() {
            errors.extend(reference(
                format!("findings[{index}].merged_from[{origin_index}]"),
                origin,
            ));
        }
    }
    for (index, dropped) in verified.dropped.iter().enumerate() {
        if let Err(message) = head.resolve(&dropped.refuted_by) {
            errors.push(Diagnostic::new(
                format!("{FILE}: dropped[{index}].refuted_by"),
                message,
            ));
        }
        errors.extend(reference(format!("dropped[{index}].from"), &dropped.from));
    }
    for (index, first) in repeats(&verified.findings, |finding| finding.id) {
        errors.push(Diagnostic::new(
            format!("{FILE}: findings[{index}]"),
            format!("id is already used by findings[{first}]"),
        ));
    }
    for (origin, described) in expected {
        match uses.get(origin).map(Vec::as_slice) {
            None | Some([]) => errors.push(Diagnostic::new(
                FILE,
                format!("{described} is neither merged into a kept finding nor dropped"),
            )),
            Some([_]) => {}
            Some(places) => errors.push(Diagnostic::new(
                FILE,
                format!(
                    "{described} appears {} times ({}); it must appear exactly once",
                    places.len(),
                    places.join(", ")
                ),
            )),
        }
    }
    errors
}

fn unknown(origin: &Origin) -> String {
    match origin {
        Origin::Reviewer(id) => format!("{id} is not a finding in any reviews/*.json"),
        Origin::Criterion(id) => {
            format!("{id} is not a not_met or partly criterion in problem.json")
        }
        Origin::Scope(id) => format!("{id} is not a scope_creep entry in problem.json"),
    }
}
