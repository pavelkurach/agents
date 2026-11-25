//! `reviews/<n>.json`: one reviewer's impact ledger and findings.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde::Deserialize;

use crate::diff::{Change, HeadFiles};
use crate::model::{
    Diagnostic, FindingId, RepoPath, Severity, Site, Sites, SuspicionId, Text, repeats,
};
use crate::proof::Proof;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: FindingId,
    pub title: Text,
    pub severity: Severity,
    pub sites: Sites,
    pub quote: String,
    pub breaks: Text,
    pub fix: Text,
    pub rule: Option<String>,
    pub proof: Option<Proof>,
}

const FLOOR: [&str; 9] = [
    "size",
    "boundaries",
    "absence",
    "shape",
    "variants",
    "state",
    "failure",
    "concurrency",
    "vs_base",
];

/// A unit's edge-case matrix: every floor dimension answered, plus any the reviewer added.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "BTreeMap<String, Text>")]
pub struct Cases(BTreeMap<String, Text>);

impl TryFrom<BTreeMap<String, Text>> for Cases {
    type Error = String;
    fn try_from(cases: BTreeMap<String, Text>) -> Result<Self, String> {
        let missing: Vec<&str> = FLOOR
            .into_iter()
            .filter(|dimension| !cases.contains_key(*dimension))
            .collect();
        if missing.is_empty() {
            Ok(Self(cases))
        } else {
            Err(format!(
                "cases missing floor dimensions: {} \u{2014} answer each with the case walked and its outcome, or n/a and why",
                missing.join(", ")
            ))
        }
    }
}

/// What a reviewer could not prove: all four parts tell the investigator what to settle.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suspicion {
    pub id: SuspicionId,
    pub claim: Text,
    pub evidence: Text,
    pub open_question: Text,
    pub tried: Text,
}

/// What the reviewer ran against a unit's riskiest cells, or why it could not run them here.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawProbe")]
pub enum Probe {
    Ran { cmd: Text, output: Text },
    NotRun { why: Text },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProbe {
    cmd: Option<Text>,
    output: Option<Text>,
    why_not_run: Option<Text>,
}

impl TryFrom<RawProbe> for Probe {
    type Error = String;
    fn try_from(raw: RawProbe) -> Result<Self, String> {
        match (raw.cmd, raw.output, raw.why_not_run) {
            (Some(cmd), Some(output), None) => Ok(Self::Ran { cmd, output }),
            (None, None, Some(why)) => Ok(Self::NotRun { why }),
            _ => Err(
                "probe is either {\"cmd\", \"output\"} (what ran and what it printed) or {\"why_not_run\"}"
                    .to_owned(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    Contract(Text),
    Unit(Text, Cases, Probe),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Fine,
    Finding(FindingId),
    Suspect(Suspicion),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawEntry")]
pub struct LedgerEntry {
    pub subject: Subject,
    pub site: Site,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Contract,
    Unit,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawVerdict {
    Fine,
    Finding,
    Suspect,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    kind: Kind,
    subject: Text,
    site: Site,
    verdict: RawVerdict,
    finding: Option<FindingId>,
    cases: Option<Cases>,
    probe: Option<Probe>,
    suspicion: Option<Suspicion>,
}

impl TryFrom<RawEntry> for LedgerEntry {
    type Error = String;
    fn try_from(raw: RawEntry) -> Result<Self, String> {
        let subject = match (raw.kind, raw.cases, raw.probe) {
            (Kind::Contract, None, None) => Subject::Contract(raw.subject),
            (Kind::Unit, Some(_), Some(Probe::NotRun { .. }))
                if matches!(raw.verdict, RawVerdict::Fine) =>
            {
                return Err(
                    "verdict `fine` needs a probe that ran; with probe.why_not_run the verdict is `suspect`"
                        .to_owned(),
                );
            }
            (Kind::Unit, Some(cases), Some(probe)) => Subject::Unit(raw.subject, cases, probe),
            (Kind::Unit, None, _) => return Err("a unit entry needs cases".to_owned()),
            (Kind::Unit, Some(_), None) => {
                return Err(
                    "a unit entry needs probe: {\"cmd\", \"output\"} of what ran, or {\"why_not_run\"}"
                        .to_owned(),
                );
            }
            (Kind::Contract, Some(_), _) => {
                return Err("cases is only valid on a unit entry".to_owned());
            }
            (Kind::Contract, None, Some(_)) => {
                return Err("probe is only valid on a unit entry".to_owned());
            }
        };
        let verdict = match (raw.verdict, raw.finding, raw.suspicion) {
            (RawVerdict::Fine, None, None) => Verdict::Fine,
            (RawVerdict::Finding, Some(id), None) => Verdict::Finding(id),
            (RawVerdict::Suspect, None, Some(suspicion)) => Verdict::Suspect(suspicion),
            (RawVerdict::Finding, None, _) => {
                return Err("verdict `finding` needs the finding id it produced".to_owned());
            }
            (RawVerdict::Suspect, _, None) => {
                return Err("verdict `suspect` needs the suspicion it raised".to_owned());
            }
            (RawVerdict::Fine | RawVerdict::Suspect, Some(id), _) => {
                return Err(format!(
                    "finding {id} is only valid with verdict `finding`; set it to null"
                ));
            }
            (RawVerdict::Fine | RawVerdict::Finding, _, Some(_)) => {
                return Err("suspicion is only valid with verdict `suspect`".to_owned());
            }
        };
        Ok(Self {
            subject,
            site: raw.site,
            verdict,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub reviewer: NonZeroU32,
    pub lens: Option<Text>,
    pub ledger: Vec<LedgerEntry>,
    pub findings: Vec<Finding>,
}

impl Review {
    /// The investigator's file: findings it confirmed from suspicions, not a review of the change.
    pub fn is_investigator(&self) -> bool {
        self.lens
            .as_ref()
            .is_some_and(|lens| lens.as_str() == "investigator")
    }

    /// Every suspicion this reviewer raised, with the site it was raised at.
    pub fn suspicions(&self) -> impl Iterator<Item = (&Site, &Suspicion)> {
        self.ledger.iter().filter_map(|entry| match &entry.verdict {
            Verdict::Suspect(suspicion) => Some((&entry.site, suspicion)),
            Verdict::Fine | Verdict::Finding(_) => None,
        })
    }

    pub fn cited_paths(&self) -> impl Iterator<Item = &RepoPath> {
        let finding_sites = self.findings.iter().flat_map(|finding| finding.sites.all());
        let ledger_sites = self.ledger.iter().map(|entry| &entry.site);
        finding_sites.chain(ledger_sites).map(|site| &site.path)
    }
}

/// Sites that resolve at head, a quote near the first one, and a rule that exists.
pub fn check_finding(
    at: &str,
    sites: &Sites,
    quote: &str,
    rule: Option<&str>,
    rules: &BTreeSet<String>,
    head: &HeadFiles,
) -> Vec<Diagnostic> {
    let unresolved = sites.all().enumerate().filter_map(|(index, site)| {
        let message = head.resolve(site).err()?;
        Some(Diagnostic::new(format!("{at}.sites[{index}]"), message))
    });
    let misquoted = head
        .quote_near(sites.first(), quote)
        .err()
        .map(|message| Diagnostic::new(at, message));
    let unknown_rule = rule.filter(|rule| !rules.contains(*rule)).map(|rule| {
        Diagnostic::new(
            at,
            format!("rule {rule} does not exist; cite an id from `revy-review rules`"),
        )
    });
    unresolved.chain(misquoted).chain(unknown_rule).collect()
}

/// Everything one review file must satisfy beyond its schema.
pub fn check(
    file: &str,
    stem: &str,
    review: &Review,
    owes_coverage: bool,
    rules: &BTreeSet<String>,
    change: &Change,
    head: &HeadFiles,
) -> Vec<Diagnostic> {
    let n = review.reviewer;
    let mut errors = Vec::new();
    if stem != n.to_string() {
        errors.push(Diagnostic::new(
            format!("{file}: reviewer"),
            format!("reviewer {n} does not match the file name {stem}.json; reviewer n writes reviews/<n>.json"),
        ));
    }
    for (index, finding) in review.findings.iter().enumerate() {
        let at = format!("{file}: findings[{index}]");
        if finding.id.reviewer != n {
            errors.push(Diagnostic::new(
                &at,
                format!(
                    "id {} does not belong to reviewer {n}; use {n}.<k>",
                    finding.id
                ),
            ));
        }
        errors.extend(check_finding(
            &at,
            &finding.sites,
            &finding.quote,
            finding.rule.as_deref(),
            rules,
            head,
        ));
    }
    for (index, first) in repeats(&review.findings, |finding| finding.id) {
        errors.push(Diagnostic::new(
            format!("{file}: findings[{index}]"),
            format!("id is already used by findings[{first}]"),
        ));
    }
    let ids: BTreeSet<FindingId> = review.findings.iter().map(|finding| finding.id).collect();
    let mut raised: BTreeMap<SuspicionId, usize> = BTreeMap::new();
    for (index, entry) in review.ledger.iter().enumerate() {
        let at = format!("{file}: ledger[{index}]");
        let mut error = |message: String| errors.push(Diagnostic::new(&at, message));
        if let Err(message) = head.resolve(&entry.site) {
            error(message);
        }
        match &entry.verdict {
            Verdict::Finding(id) if !ids.contains(id) => {
                error(format!(
                    "verdict `finding` names {id}, which is not a finding in this file"
                ));
            }
            Verdict::Suspect(Suspicion { id, .. }) => {
                if id.reviewer != n {
                    error(format!(
                        "suspicion id {id} does not belong to reviewer {n}; use {n}.s<k>"
                    ));
                }
                if let Some(first) = raised.insert(*id, index) {
                    error(format!(
                        "suspicion id {id} is already used by ledger[{first}]"
                    ));
                }
            }
            Verdict::Fine | Verdict::Finding(_) => {}
        }
    }
    if owes_coverage {
        let covered: BTreeSet<&RepoPath> =
            review.ledger.iter().map(|entry| &entry.site.path).collect();
        for path in change.files().filter(|path| !covered.contains(path)) {
            errors.push(Diagnostic::new(
                format!("{file}: ledger"),
                format!("changed file {path} has no ledger entry"),
            ));
        }
    }
    errors
}
