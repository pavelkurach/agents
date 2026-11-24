//! `problem.json`: the problem gate's statement, criteria and verdicts.

use serde::Deserialize;

use crate::diff::HeadFiles;
use crate::model::{CriterionId, Diagnostic, RepoPath, ScopeCreepId, Site, Sites, Text, repeats};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionVerdict {
    Met,
    Partly,
    NotMet,
    Unverifiable,
}

impl CriterionVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Met => "met",
            Self::Partly => "partly",
            Self::NotMet => "not_met",
            Self::Unverifiable => "unverifiable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawCriterion")]
pub struct Criterion {
    pub id: CriterionId,
    pub text: Text,
    /// Quoted from a source; implied criteria are not core.
    pub core: bool,
    pub verdict: CriterionVerdict,
    pub evidence: Vec<Site>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCriterion {
    id: CriterionId,
    text: Text,
    quote: Option<Text>,
    implied: bool,
    verdict: CriterionVerdict,
    evidence: Vec<Site>,
}

impl TryFrom<RawCriterion> for Criterion {
    type Error = String;
    fn try_from(raw: RawCriterion) -> Result<Self, String> {
        if raw.quote.is_some() == raw.implied {
            return Err(format!(
                "{}: quote the source with implied false, or set quote null with implied true",
                raw.id
            ));
        }
        Ok(Self {
            id: raw.id,
            text: raw.text,
            core: !raw.implied,
            verdict: raw.verdict,
            evidence: raw.evidence,
        })
    }
}

/// Unrelated changes sharing one cause.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeCreep {
    pub id: ScopeCreepId,
    pub sites: Sites,
    pub why: Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    pub statement: Text,
    pub source: Text,
    pub criteria: Vec<Criterion>,
    pub out_of_scope: Vec<Text>,
    pub scope_creep: Vec<ScopeCreep>,
    pub blocks: bool,
}

impl Problem {
    /// Why the gate blocks, if it does: a core criterion `not_met`, or none `met`.
    pub fn block_reason(&self) -> Option<String> {
        if let Some(core) = self
            .criteria
            .iter()
            .find(|c| c.core && c.verdict == CriterionVerdict::NotMet)
        {
            return Some(format!("core criterion {} is not_met", core.id));
        }
        let none_met = !self
            .criteria
            .iter()
            .any(|c| c.verdict == CriterionVerdict::Met);
        none_met.then(|| "no criterion is met".to_owned())
    }

    pub fn cited_paths(&self) -> impl Iterator<Item = &RepoPath> {
        let evidence = self.criteria.iter().flat_map(|c| &c.evidence);
        let creep = self.scope_creep.iter().flat_map(|s| s.sites.all());
        evidence.chain(creep).map(|site| &site.path)
    }
}

pub fn check(file: &str, problem: &Problem, head: &HeadFiles) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let mut error = |at: String, message: String| errors.push(Diagnostic::new(at, message));
    if problem.criteria.is_empty() {
        error(
            format!("{file}: criteria"),
            "no criteria; list at least one, quoted or implied".to_owned(),
        );
    }
    for (index, criterion) in problem.criteria.iter().enumerate() {
        let needs_evidence = matches!(
            criterion.verdict,
            CriterionVerdict::Met | CriterionVerdict::Partly
        );
        if needs_evidence && criterion.evidence.is_empty() {
            error(
                format!("{file}: criteria[{index}]"),
                format!(
                    "verdict {} needs at least one evidence site",
                    criterion.verdict.as_str()
                ),
            );
        }
        for (site_index, site) in criterion.evidence.iter().enumerate() {
            if let Err(message) = head.resolve(site) {
                error(
                    format!("{file}: criteria[{index}].evidence[{site_index}]"),
                    message,
                );
            }
        }
    }
    for (index, creep) in problem.scope_creep.iter().enumerate() {
        for (site_index, site) in creep.sites.all().enumerate() {
            if let Err(message) = head.resolve(site) {
                error(
                    format!("{file}: scope_creep[{index}].sites[{site_index}]"),
                    message,
                );
            }
        }
    }
    for (index, first) in repeats(&problem.criteria, |c| c.id) {
        error(
            format!("{file}: criteria[{index}]"),
            format!("id is already used by criteria[{first}]"),
        );
    }
    for (index, first) in repeats(&problem.scope_creep, |s| s.id) {
        error(
            format!("{file}: scope_creep[{index}]"),
            format!("id is already used by scope_creep[{first}]"),
        );
    }
    let reason = problem.block_reason();
    if reason.is_some() != problem.blocks {
        let detail =
            reason.unwrap_or_else(|| "no core criterion is not_met and one is met".to_owned());
        error(
            format!("{file}: blocks"),
            format!(
                "is {} but the criteria give {} ({detail})",
                problem.blocks, !problem.blocks
            ),
        );
    }
    errors
}
