//! The typed review vocabulary and plan regions.
//!
//! Severities, findings, review kinds, lane verdicts, consumer checks, and the
//! planner's triage and the user's rulings: the payloads a review wave produces
//! and the plan state carries. A [`PlanRegion`] names one fingerprintable part of
//! a candidate; [`fingerprints`] digests each so a correction can be checked to
//! have actually changed the regions it claims (design §8.3).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{
    AcId, ClaimId, DecisionId, FindingId, MrId, RequirementId, RiskId, UncertaintyId,
};
use crate::model::content::SpecialistFocus;
use crate::model::impact::BlastLane;
use crate::primitives::{NonEmptyText, OneOrMore, Sha256Digest};
use crate::state::RawCandidate;

/// A named, independently fingerprintable region of a plan candidate.
///
/// Closed: a finding, a triage, and a correction all express themselves in terms
/// of these parts. One region per brief, decision, claim, uncertainty, impact
/// subject, coverage lane, changed symbol, requirement, criterion, risk, and
/// delivery item (design §6.6).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "region", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanRegion {
    Brief,
    Decision { id: DecisionId },
    Claim { id: ClaimId },
    Uncertainty { id: UncertaintyId },
    Impact { subject: NonEmptyText },
    Coverage { lane: BlastLane },
    ChangedSymbol { symbol: NonEmptyText },
    Requirement { id: RequirementId },
    Criterion { id: AcId },
    Risk { id: RiskId },
    DeliveryItem { id: MrId },
}

/// The content digest of a [`PlanRegion`], used to detect whether a correction
/// actually changed the region it claims to fix.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionFingerprint(Sha256Digest);

impl RegionFingerprint {
    pub(crate) fn of(bytes: &[u8]) -> Self {
        Self(Sha256Digest::hash(bytes))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn fingerprint<T: Serialize>(value: &T) -> RegionFingerprint {
    RegionFingerprint::of(&serde_json::to_vec(value).unwrap_or_default())
}

/// The canonical fingerprint of every region a candidate carries. Two candidates
/// are compared region-by-region to prove a correction touched what it claims.
pub fn fingerprints(candidate: &RawCandidate) -> BTreeMap<PlanRegion, RegionFingerprint> {
    let mut map = BTreeMap::new();
    map.insert(PlanRegion::Brief, fingerprint(&candidate.brief));
    for decision in &candidate.decisions {
        map.insert(
            PlanRegion::Decision {
                id: decision.id.clone(),
            },
            fingerprint(decision),
        );
    }
    for claim in &candidate.exploration.claims {
        map.insert(
            PlanRegion::Claim {
                id: claim.id.clone(),
            },
            fingerprint(claim),
        );
    }
    for uncertainty in &candidate.exploration.uncertainties {
        map.insert(
            PlanRegion::Uncertainty {
                id: uncertainty.id.clone(),
            },
            fingerprint(uncertainty),
        );
    }
    for assessment in &candidate.exploration.impact {
        map.insert(
            PlanRegion::Impact {
                subject: assessment.subject.clone(),
            },
            fingerprint(assessment),
        );
    }
    for entry in &candidate.exploration.coverage {
        map.insert(
            PlanRegion::Coverage { lane: entry.lane },
            fingerprint(entry),
        );
    }
    for changed in &candidate.content.changed_symbols {
        map.insert(
            PlanRegion::ChangedSymbol {
                symbol: changed.symbol.clone(),
            },
            fingerprint(changed),
        );
    }
    for risk in &candidate.content.risks {
        map.insert(
            PlanRegion::Risk {
                id: risk.id().clone(),
            },
            fingerprint(risk),
        );
    }
    for requirement in &candidate.delivery.requirements {
        map.insert(
            PlanRegion::Requirement {
                id: requirement.id.clone(),
            },
            fingerprint(requirement),
        );
    }
    for criterion in &candidate.delivery.acceptance_criteria {
        map.insert(
            PlanRegion::Criterion {
                id: criterion.id.clone(),
            },
            fingerprint(criterion),
        );
    }
    for mr in &candidate.delivery.mrs {
        map.insert(
            PlanRegion::DeliveryItem { id: mr.id.clone() },
            fingerprint(mr),
        );
    }
    map
}

/// The set of every region a candidate carries. Derived from [`fingerprints`], so
/// the two can never disagree on the region universe.
pub fn regions(candidate: &RawCandidate) -> BTreeSet<PlanRegion> {
    fingerprints(candidate).into_keys().collect()
}

/// How badly a finding blocks the plan. `Blocking` and `Major` are *material*:
/// the planner must triage each before the candidate can re-enter review.
/// `Advisory` is recorded but never forces a correction.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Blocking,
    Major,
    Advisory,
}

impl Severity {
    /// Whether this severity forces a triage before the plan can clear review.
    /// Only `Blocking` and `Major` are material.
    pub const fn is_material(self) -> bool {
        matches!(self, Self::Blocking | Self::Major)
    }
}

/// One review a wave can run. The integrated review covers the whole candidate; a
/// specialist review covers one focus.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewKind {
    Integrated,
    Specialist { focus: SpecialistFocus },
}

/// One of the fixed review lenses a critic applies to a plan candidate.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReviewLens {
    FactCheck,
    Evidence,
    Completeness,
    AcceptanceCriteria,
    Contract,
    Summary,
    /// Cross-checks the plan for internal contradiction.
    Consistency,
    /// A consumer of a changed symbol that the integrated review found unaccounted
    /// for (a `ConsumerCheck::Missing`).
    Consumers,
    // The specialist lenses, one per `SpecialistFocus`. A specialist review's
    // finding names the lens matching the focus it ran under, so a `performance`
    // or `public_api` finding is as expressible as an integrated one. The names
    // mirror `SpecialistFocus` on the wire.
    Concurrency,
    Migration,
    Security,
    ProtocolCompatibility,
    Performance,
    PublicApi,
}

/// A single reviewer finding: the lens that raised it, how badly it blocks the
/// plan, the regions it concerns, and the rejected finding it re-raises, if any.
///
/// A reviewer never records a resolution — only a [`Triage`] carries that (design
/// decision 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: FindingId,
    pub lens: ReviewLens,
    pub severity: Severity,
    pub statement: NonEmptyText,
    pub regions: OneOrMore<PlanRegion>,
    /// A finding rejected in an earlier round that this one re-raises; the wave
    /// then escalates to the user rather than auto-correcting (design §8.2).
    #[serde(default)]
    pub reraises: Option<FindingId>,
}

impl Finding {
    /// Whether this finding is material (blocking or major).
    pub const fn is_material(&self) -> bool {
        self.severity.is_material()
    }
}

/// One blast lane's verdict from the integrated review: confirmed present, or
/// contested by a finding it must name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaneVerdict {
    pub lane: BlastLane,
    pub verdict: LaneVerdictKind,
}

/// Whether a reviewed lane is confirmed, or contested by a named finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LaneVerdictKind {
    Confirmed,
    Contested { finding: FindingId },
}

/// The verdict of a consumer check over one modified or removed symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConsumerVerdict {
    AllAccounted,
    Missing { finding: FindingId },
}

/// The integrated review's check that every consumer of a modified or removed
/// symbol is accounted for: the symbol, the tool/query used, how many references
/// it found, and the verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerCheck {
    pub symbol: NonEmptyText,
    pub method: NonEmptyText,
    pub references_found: u32,
    pub verdict: ConsumerVerdict,
}

/// A review that has run: the integrated review with its lane verdicts and
/// consumer checks, or a specialist review over one focus. Findings only —
/// resolution belongs to the planner's triage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompletedReview {
    Integrated {
        findings: Vec<Finding>,
        lanes: Vec<LaneVerdict>,
        consumers: Vec<ConsumerCheck>,
    },
    Specialist {
        focus: SpecialistFocus,
        findings: Vec<Finding>,
    },
}

impl CompletedReview {
    /// The findings this review raised.
    pub fn findings(&self) -> &[Finding] {
        match self {
            Self::Integrated { findings, .. } | Self::Specialist { findings, .. } => findings,
        }
    }

    /// The review kind this discharged.
    pub const fn kind(&self) -> ReviewKind {
        match self {
            Self::Integrated { .. } => ReviewKind::Integrated,
            Self::Specialist { focus, .. } => ReviewKind::Specialist { focus: *focus },
        }
    }
}

/// The planner's disposition of one material finding: fix it in named regions, or
/// reject it with a rationale the next reviewer sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TriageOutcome {
    Fix { affected: OneOrMore<PlanRegion> },
    Reject { rationale: NonEmptyText },
}

/// One material finding paired with how the planner triaged it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Triage {
    pub finding: FindingId,
    pub outcome: TriageOutcome,
}

/// The user's ruling on one escalated finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RulingVerdict {
    MustFix,
    Dismiss,
    Accept,
}

/// One escalated finding paired with the user's ruling on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ruling {
    pub finding: FindingId,
    pub verdict: RulingVerdict,
}

/// Why a wave escalated to the user rather than auto-correcting: the automatic
/// review budget is exhausted, or a rejected finding was re-raised (design §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewEscalation {
    BudgetExhausted,
    Reraised { findings: OneOrMore<FindingId> },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn test_consistency_lens_round_trips_on_the_wire() {
        assert_eq!(
            serde_json::to_value(ReviewLens::Consistency).expect("serializes"),
            serde_json::json!("consistency")
        );
    }

    #[test]
    fn test_finding_has_no_resolution_field() {
        // A reviewer-authored resolution is an unknown field: reviewers return
        // findings only (design decision 1).
        let json = r#"{"id":"finding-1","lens":"fact_check","severity":"blocking","statement":"x","regions":[{"region":"brief"}],"resolution":{"state":"rejected"}}"#;
        serde_json::from_str::<Finding>(json).expect_err("a finding carries no resolution field");
    }

    #[test]
    fn test_a_specialist_finding_names_the_lens_matching_its_focus() {
        // Every SpecialistFocus has a mirror ReviewLens, so a specialist review's
        // finding can name its focus (here `performance`) as the lens that raised
        // it and still parse.
        let json = r#"{"kind":"specialist","focus":"performance","findings":[{"id":"finding-1","lens":"performance","severity":"major","statement":"A hot path allocates per call.","regions":[{"region":"brief"}]}]}"#;
        let review: CompletedReview = serde_json::from_str(json)
            .expect("a specialist finding may name its focus as the lens");
        let lens = review.findings().first().map(|finding| finding.lens);
        assert_eq!(lens, Some(ReviewLens::Performance));
    }

    #[test]
    fn test_finding_parses_a_brief_region() {
        let json = r#"{"id":"finding-1","lens":"consistency","severity":"major","statement":"x","regions":[{"region":"brief"}]}"#;
        let finding: Finding = serde_json::from_str(json).expect("a brief-region finding parses");
        assert_eq!(finding.regions.as_slice(), &[PlanRegion::Brief]);
    }
}
