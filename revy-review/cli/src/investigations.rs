//! `investigations.json`: the investigator's resolution of every reviewer suspicion.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::model::{Diagnostic, FindingId, Site, SuspicionId, Text, repeats};
use crate::review::Review;

pub const FILE: &str = "investigations.json";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    Confirmed,
    Refuted,
    Unresolved,
}

/// How a suspicion ended; each variant carries exactly what it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The finding the investigator appended to its own review file.
    Confirmed(FindingId),
    Refuted,
    /// How a human settles it quickly.
    Unresolved(Text),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    suspicion: SuspicionId,
    outcome: Outcome,
    reason: Text,
    evidence: Text,
    finding: Option<FindingId>,
    check: Option<Text>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawItem")]
pub struct Item {
    pub suspicion: SuspicionId,
    pub reason: Text,
    pub evidence: Text,
    pub resolution: Resolution,
}

impl TryFrom<RawItem> for Item {
    type Error = String;
    fn try_from(raw: RawItem) -> Result<Self, String> {
        let id = raw.suspicion;
        let resolution = match (raw.outcome, raw.finding, raw.check) {
            (Outcome::Confirmed, Some(finding), None) => Resolution::Confirmed(finding),
            (Outcome::Refuted, None, None) => Resolution::Refuted,
            (Outcome::Unresolved, None, Some(check)) => Resolution::Unresolved(check),
            (Outcome::Confirmed, None, _) => {
                return Err(format!(
                    "{id}: outcome `confirmed` needs the finding id appended to the investigator's review file"
                ));
            }
            (Outcome::Unresolved, None, None) => {
                return Err(format!(
                    "{id}: outcome `unresolved` needs a check a human can run quickly"
                ));
            }
            _ => {
                return Err(format!(
                    "{id}: finding is only valid with `confirmed` and check only with `unresolved`; set the other to null"
                ));
            }
        };
        Ok(Self {
            suspicion: id,
            reason: raw.reason,
            evidence: raw.evidence,
            resolution,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Investigations {
    pub items: Vec<Item>,
}

/// Every suspicion raised in `reviews`, in id order.
pub fn raised(
    reviews: &[(String, Review)],
) -> BTreeMap<SuspicionId, (&Site, &crate::review::Suspicion)> {
    reviews
        .iter()
        .flat_map(|(_, review)| review.suspicions())
        .map(|(site, suspicion)| (suspicion.id, (site, suspicion)))
        .collect()
}

/// Each suspicion resolved exactly once, no unknown ids, and every confirmed
/// finding in the investigator's own review file.
pub fn check(investigations: &Investigations, reviews: &[(String, Review)]) -> Vec<Diagnostic> {
    let raised = raised(reviews);
    let findings = |investigator: bool| -> BTreeSet<FindingId> {
        reviews
            .iter()
            .filter(|(_, review)| review.is_investigator() == investigator)
            .flat_map(|(_, review)| review.findings.iter().map(|finding| finding.id))
            .collect()
    };
    let (investigator, reviewers) = (findings(true), findings(false));
    let mut errors = Vec::new();
    for (index, item) in investigations.items.iter().enumerate() {
        let at = format!("{FILE}: items[{index}]");
        if !raised.contains_key(&item.suspicion) {
            errors.push(Diagnostic::new(
                &at,
                format!(
                    "{} is not a suspicion in any reviews/*.json",
                    item.suspicion
                ),
            ));
        }
        if let Resolution::Confirmed(finding) = &item.resolution
            && !investigator.contains(finding)
        {
            let message = if reviewers.contains(finding) {
                format!(
                    "confirmed names finding {finding}, which belongs to a reviewer; confirmed must name a finding in the review file with lens `investigator`"
                )
            } else {
                format!(
                    "confirmed names finding {finding}, which is not a finding in any reviews/*.json; append it to reviews/<m>.json first"
                )
            };
            errors.push(Diagnostic::new(at, message));
        }
    }
    for (index, first) in repeats(&investigations.items, |item| item.suspicion) {
        errors.push(Diagnostic::new(
            format!("{FILE}: items[{index}]"),
            format!("suspicion is already resolved by items[{first}]"),
        ));
    }
    let resolved: BTreeSet<SuspicionId> = investigations
        .items
        .iter()
        .map(|item| item.suspicion)
        .collect();
    errors.extend(raised.keys().filter(|id| !resolved.contains(id)).map(|id| {
        Diagnostic::new(
            FILE,
            format!("suspicion {id} has no item; the investigator must resolve every suspicion exactly once"),
        )
    }));
    errors
}

/// A suspicion the investigator could not settle, ready to show the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub claim: Text,
    pub site: Site,
    pub evidence: Text,
    pub open_question: Text,
    pub check: Text,
}

/// Unresolved suspicions in id order, joined with the reviewer's claim and site.
pub fn unresolved(
    reviews: &[(String, Review)],
    investigations: &Investigations,
) -> Vec<Unresolved> {
    let raised = raised(reviews);
    let mut open: Vec<(SuspicionId, Unresolved)> = investigations
        .items
        .iter()
        .filter_map(|item| {
            let Resolution::Unresolved(check) = &item.resolution else {
                return None;
            };
            let (site, suspicion) = raised.get(&item.suspicion)?;
            Some((
                item.suspicion,
                Unresolved {
                    claim: suspicion.claim.clone(),
                    site: (*site).clone(),
                    evidence: item.evidence.clone(),
                    open_question: suspicion.open_question.clone(),
                    check: check.clone(),
                },
            ))
        })
        .collect();
    open.sort_by_key(|(id, _)| *id);
    open.into_iter().map(|(_, unresolved)| unresolved).collect()
}
