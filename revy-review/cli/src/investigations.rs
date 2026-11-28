//! `investigations/<n>.json`: investigator n's resolution of the suspicions it was given;
//! together the files resolve every reviewer suspicion exactly once.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde::Deserialize;

use crate::model::{Diagnostic, FindingId, Site, SuspicionId, Text};
use crate::review::{Review, Suspicion};

pub const DIR: &str = "investigations";

/// The one file all suspicions went to before investigators ran in parallel.
pub const LEGACY_FILE: &str = "investigations.json";
pub const LEGACY_MIGRATION: &str = "old single-file format; move it to investigations/<n>.json, n = the investigator's reviewer number";

/// Every investigator's file, by investigator number.
pub type Shards = BTreeMap<NonZeroU32, Investigations>;

pub fn file(investigator: NonZeroU32) -> String {
    format!("{DIR}/{investigator}.json")
}

/// The investigator number a file stem names, spelled as reviewer numbers are, so two
/// files never name the same investigator.
pub fn number(stem: &str) -> Result<NonZeroU32, String> {
    stem.parse::<NonZeroU32>()
        .ok()
        .filter(|n| n.to_string() == stem)
        .ok_or_else(|| format!("not an investigator number; investigator n writes {DIR}/<n>.json"))
}

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

/// Every suspicion raised in `reviews`, investigators' own doubts included, in id order.
pub fn raised(reviews: &[(String, Review)]) -> BTreeMap<SuspicionId, (&Site, &Suspicion)> {
    reviews
        .iter()
        .flat_map(|(_, review)| review.suspicions())
        .map(|(site, suspicion)| (suspicion.id, (site, suspicion)))
        .collect()
}

/// Whose review file holds a finding.
#[derive(Debug, Clone, Copy)]
enum Owner {
    Reviewer,
    Investigator(NonZeroU32),
}

/// `(investigator, items[index] location, item)` across every file, in investigator order.
fn items(shards: &Shards) -> impl Iterator<Item = (NonZeroU32, String, &Item)> {
    shards.iter().flat_map(|(&investigator, doc)| {
        doc.items.iter().enumerate().map(move |(index, item)| {
            (
                investigator,
                format!("{}: items[{index}]", file(investigator)),
                item,
            )
        })
    })
}

/// Checks the union of every investigator's file.
///
/// Each suspicion is resolved exactly once, no id is unknown, an investigator's own
/// suspicion is resolved in its own file, and every confirmed finding is in the review file
/// of the investigator that confirmed it.
pub fn check(shards: &Shards, reviews: &[(String, Review)]) -> Vec<Diagnostic> {
    let raised = raised(reviews);
    let investigators: BTreeSet<NonZeroU32> = reviews
        .iter()
        .filter(|(_, review)| review.is_investigator())
        .map(|(_, review)| review.reviewer)
        .collect();
    let owners: BTreeMap<FindingId, Owner> = reviews
        .iter()
        .flat_map(|(_, review)| {
            let owner = if review.is_investigator() {
                Owner::Investigator(review.reviewer)
            } else {
                Owner::Reviewer
            };
            review
                .findings
                .iter()
                .map(move |finding| (finding.id, owner))
        })
        .collect();
    let mut errors = Vec::new();
    let mut resolved: BTreeMap<SuspicionId, String> = BTreeMap::new();
    for (investigator, at, item) in items(shards) {
        if !raised.contains_key(&item.suspicion) {
            errors.push(Diagnostic::new(
                &at,
                format!(
                    "{} is not a suspicion in any reviews/*.json",
                    item.suspicion
                ),
            ));
        }
        let raised_by = item.suspicion.reviewer;
        if investigators.contains(&raised_by) && raised_by != investigator {
            errors.push(Diagnostic::new(
                &at,
                format!(
                    "investigator {raised_by} raised suspicion {}; resolve it in its own {}",
                    item.suspicion,
                    file(raised_by)
                ),
            ));
        }
        if let Resolution::Confirmed(finding) = &item.resolution
            && let Some(message) = misplaced(*finding, investigator, owners.get(finding))
        {
            errors.push(Diagnostic::new(&at, message));
        }
        if let Some(first) = resolved.get(&item.suspicion) {
            errors.push(Diagnostic::new(
                &at,
                format!(
                    "suspicion {} is already resolved by {first}",
                    item.suspicion
                ),
            ));
        } else {
            resolved.insert(item.suspicion, at);
        }
    }
    errors.extend(raised.keys().filter(|id| !resolved.contains_key(id)).map(|id| {
        Diagnostic::new(
            DIR,
            format!("suspicion {id} has no item in any {DIR}/<n>.json; every suspicion is resolved exactly once"),
        )
    }));
    errors
}

/// Why `finding`, confirmed by `investigator`, is not where that investigator must append it.
fn misplaced(
    finding: FindingId,
    investigator: NonZeroU32,
    owner: Option<&Owner>,
) -> Option<String> {
    match owner {
        Some(Owner::Investigator(n)) if *n == investigator => None,
        Some(Owner::Investigator(n)) => Some(format!(
            "confirmed names finding {finding} of investigator {n}; investigator {investigator} confirms into its own reviews/{investigator}.json"
        )),
        Some(Owner::Reviewer) => Some(format!(
            "confirmed names finding {finding}, which belongs to a reviewer; confirmed must name a finding in the review file with lens `investigator`"
        )),
        None => Some(format!(
            "confirmed names finding {finding}, which is not a finding in any reviews/*.json; append it to reviews/{investigator}.json (lens `investigator`) first"
        )),
    }
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
pub fn unresolved(reviews: &[(String, Review)], shards: &Shards) -> Vec<Unresolved> {
    let raised = raised(reviews);
    let mut open: Vec<(SuspicionId, Unresolved)> = shards
        .values()
        .flat_map(|doc| &doc.items)
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
