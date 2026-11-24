//! The serialized run state as a sum type, split into a raw (deserializable)
//! form and a proven form.
//!
//! `RawRunState` is the schema/deserialization source. `RunState` is built only
//! by [`crate::document::ParsedPlan::parse`] or by workflow transitions; it
//! deliberately does not implement `Deserialize`. The raw/proven split is kept
//! only where a proof exists beyond parse — `Candidate` and `ExplorationContext`.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::CapabilityReport;
use crate::evidence::Evidence;
use crate::ids::{ClaimId, DecisionId, EvidenceId, FindingId, ProposalId, UncertaintyId};
use crate::model::claim::{Claim, UncertaintyDisposition, UnresolvedFact};
use crate::model::content::{Decision, DecisionOwner, DecisionState, DesignBrief, PlanContent};
use crate::model::content::{ProposedDecision, UserQuestion};
use crate::model::delivery::{DeliveryError, DeliveryGraph, DeliveryPlan};
use crate::model::discussion::Comment;
use crate::model::impact::{
    BlastLane, ImpactAssessment, ImpactCoverage, ImpactEntry, LaneCoverage,
};
use crate::model::review::{
    CompletedReview, Finding, PlanRegion, RegionFingerprint, ReviewEscalation, ReviewKind, Ruling,
    Triage, fingerprints,
};
use crate::primitives::{NonEmptyText, OneOrMore, Sha256Digest};
use crate::repository::Repository;

/// The one-based wave counter. Three automatic waves, then the user rules per
/// finding (design decision 3).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct WaveNumber(NonZeroU32);

impl WaveNumber {
    /// The number of automatic review waves before the loop escalates to the user.
    pub const MAX_AUTOMATIC: u32 = 3;

    /// The first wave.
    pub const fn first() -> Self {
        Self(NonZeroU32::MIN)
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }

    /// The next wave.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Whether this is the last automatic wave: a material wave here escalates to
    /// the user rather than running another automatic correction.
    pub const fn is_budget_exhausted(self) -> bool {
        self.0.get() >= Self::MAX_AUTOMATIC
    }
}

/// The approved capability set driving exploration: the report the user acted on
/// and the proposals they approved. Proven that every approved proposal is one the
/// report disclosed; re-proven at load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApprovedCapabilities {
    report: CapabilityReport,
    approved: Vec<ProposalId>,
}

/// An approved-capabilities set that names a proposal the report never disclosed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovedCapabilityError {
    #[error("approval names undisclosed proposal `{proposal}`")]
    UndisclosedProposal { proposal: ProposalId },
}

impl ApprovedCapabilities {
    /// Build an approved set, proving every approved proposal was disclosed by the
    /// report.
    pub fn parse(
        report: CapabilityReport,
        approved: Vec<ProposalId>,
    ) -> Result<Self, ApprovedCapabilityError> {
        let known: BTreeSet<&ProposalId> = report.proposals.iter().map(|p| &p.id).collect();
        for proposal in &approved {
            if !known.contains(proposal) {
                return Err(ApprovedCapabilityError::UndisclosedProposal {
                    proposal: proposal.clone(),
                });
            }
        }
        Ok(Self { report, approved })
    }

    pub const fn report(&self) -> &CapabilityReport {
        &self.report
    }

    pub fn approved(&self) -> &[ProposalId] {
        &self.approved
    }
}

impl<'de> Deserialize<'de> for ApprovedCapabilities {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            report: CapabilityReport,
            approved: Vec<ProposalId>,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::parse(raw.report, raw.approved).map_err(serde::de::Error::custom)
    }
}

/// The bundle of evidence, claims, uncertainties, impact, and coverage that
/// exploration produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawExplorationContext {
    pub evidence: Vec<Evidence>,
    pub claims: Vec<Claim>,
    pub uncertainties: Vec<UnresolvedFact>,
    pub impact: Vec<ImpactAssessment>,
    #[serde(default)]
    pub coverage: Vec<ImpactCoverage>,
}

/// A plan candidate under review: its brief, decisions, exploration, content, and
/// delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawCandidate {
    pub brief: DesignBrief,
    pub decisions: Vec<Decision>,
    pub exploration: RawExplorationContext,
    pub content: PlanContent,
    pub delivery: DeliveryPlan,
}

/// Progress through exploration: still collecting over the evidence gathered so
/// far, or complete with the finalized context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum RawExplorationProgress {
    Collecting { prior: RawExplorationContext },
    Complete { context: RawExplorationContext },
}

/// One completed review round, recorded in history: the wave, its reviews, the
/// planner's triage (empty for the clearing wave), and the user's rulings
/// (non-empty only when the user ruled on this wave).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewRound {
    pub wave: WaveNumber,
    pub reviews: OneOrMore<CompletedReview>,
    #[serde(default)]
    pub triage: Vec<Triage>,
    #[serde(default)]
    pub rulings: Vec<Ruling>,
}

/// The serialized run-state sum: the schema and deserialization source of truth.
/// Adjacently tagged so `serde_path_to_error` reports exact pointers inside each
/// variant's body (design §5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "state",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum RawRunState {
    AwaitingCapabilityApproval {
        report: CapabilityReport,
        /// Evidence gathered before a mid-exploration amendment; `None` at init.
        prior: Option<RawExplorationContext>,
    },
    Exploring {
        capabilities: ApprovedCapabilities,
        progress: RawExplorationProgress,
    },
    AwaitingUserDecision {
        capabilities: ApprovedCapabilities,
        context: RawExplorationContext,
        brief: DesignBrief,
        decisions: OneOrMore<ProposedDecision>,
        questions: OneOrMore<UserQuestion>,
    },
    Drafting {
        capabilities: ApprovedCapabilities,
        context: RawExplorationContext,
        brief: DesignBrief,
        decisions: Vec<Decision>,
    },
    Reviewing {
        candidate: RawCandidate,
        wave: WaveNumber,
        pending: OneOrMore<ReviewKind>,
        #[serde(default)]
        completed: Vec<CompletedReview>,
        #[serde(default)]
        history: Vec<ReviewRound>,
    },
    Correcting {
        candidate: RawCandidate,
        wave: WaveNumber,
        reviews: OneOrMore<CompletedReview>,
        #[serde(default)]
        must_fix: Vec<FindingId>,
        #[serde(default)]
        history: Vec<ReviewRound>,
    },
    AwaitingReviewDecision {
        candidate: RawCandidate,
        wave: WaveNumber,
        unresolved: OneOrMore<Finding>,
        cause: ReviewEscalation,
        #[serde(default)]
        history: Vec<ReviewRound>,
    },
    Final {
        candidate: RawCandidate,
        #[serde(default)]
        history: Vec<ReviewRound>,
        #[serde(default)]
        accepted: Vec<Finding>,
        #[serde(default)]
        comments: Vec<Comment>,
    },
}

/// The stable id an evidence item carries, regardless of its kind.
const fn evidence_id(evidence: &Evidence) -> &EvidenceId {
    match evidence {
        Evidence::Runtime(evidence) => &evidence.id,
        Evidence::Semantic(evidence) => &evidence.id,
        Evidence::Structural(evidence) => &evidence.id,
        Evidence::Textual(evidence) => &evidence.id,
        Evidence::Documentation(evidence) => &evidence.id,
        Evidence::History(evidence) => &evidence.id,
        Evidence::Inference(evidence) => &evidence.id,
    }
}

/// A cross-reference in an exploration context that does not resolve, or an id
/// that is not unique, or a dishonest sample.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExplorationContextError {
    #[error("duplicate evidence id `{id}`")]
    DuplicateEvidence { id: EvidenceId },
    #[error("duplicate claim id `{id}`")]
    DuplicateClaim { id: ClaimId },
    #[error("duplicate impact subject `{subject}`")]
    DuplicateImpactSubject { subject: NonEmptyText },
    #[error("evidence `{id}` declares a sample smaller than the items it retained")]
    DishonestSample { id: EvidenceId },
    #[error("claim evidence `{reference}` does not resolve to any evidence")]
    UnresolvedClaimEvidence { reference: EvidenceId },
    #[error("impact claim `{reference}` does not resolve to any claim")]
    UnresolvedImpactClaim { reference: ClaimId },
    #[error("impact uncertainty `{reference}` does not resolve to any uncertainty")]
    UnresolvedImpactUncertainty { reference: UncertaintyId },
}

/// A proven exploration context. Built only by parsing or workflow transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorationContext(RawExplorationContext);

impl ExplorationContext {
    pub(crate) const fn from_raw(raw: RawExplorationContext) -> Self {
        Self(raw)
    }

    pub(crate) fn into_raw(self) -> RawExplorationContext {
        self.0
    }

    pub const fn raw(&self) -> &RawExplorationContext {
        &self.0
    }

    /// The empty context: no evidence, claims, uncertainties, impact, or coverage.
    pub(crate) const fn empty() -> Self {
        Self(RawExplorationContext {
            evidence: Vec::new(),
            claims: Vec::new(),
            uncertainties: Vec::new(),
            impact: Vec::new(),
            coverage: Vec::new(),
        })
    }

    /// Prove a raw context: unique ids, every cross-reference resolved, and every
    /// declared sample honest about the items it retained.
    pub(crate) fn parse(raw: RawExplorationContext) -> Result<Self, ExplorationContextError> {
        let mut evidence_ids: BTreeSet<&EvidenceId> = BTreeSet::new();
        for evidence in &raw.evidence {
            if !evidence_ids.insert(evidence_id(evidence)) {
                return Err(ExplorationContextError::DuplicateEvidence {
                    id: evidence_id(evidence).clone(),
                });
            }
            if !evidence.sample_admits() {
                return Err(ExplorationContextError::DishonestSample {
                    id: evidence_id(evidence).clone(),
                });
            }
        }
        let mut claim_ids: BTreeSet<&ClaimId> = BTreeSet::new();
        for claim in &raw.claims {
            if !claim_ids.insert(&claim.id) {
                return Err(ExplorationContextError::DuplicateClaim {
                    id: claim.id.clone(),
                });
            }
        }
        for claim in &raw.claims {
            for reference in &claim.evidence {
                if !evidence_ids.contains(reference) {
                    return Err(ExplorationContextError::UnresolvedClaimEvidence {
                        reference: reference.clone(),
                    });
                }
            }
        }
        let mut impact_subjects: BTreeSet<&NonEmptyText> = BTreeSet::new();
        for assessment in &raw.impact {
            if !impact_subjects.insert(&assessment.subject) {
                return Err(ExplorationContextError::DuplicateImpactSubject {
                    subject: assessment.subject.clone(),
                });
            }
        }
        let uncertainty_ids: BTreeSet<&UncertaintyId> =
            raw.uncertainties.iter().map(|u| &u.id).collect();
        for assessment in &raw.impact {
            match &assessment.entry {
                ImpactEntry::Affected { claims, .. } => {
                    for reference in claims {
                        if !claim_ids.contains(reference) {
                            return Err(ExplorationContextError::UnresolvedImpactClaim {
                                reference: reference.clone(),
                            });
                        }
                    }
                }
                ImpactEntry::Uncertain { uncertainty, .. } => {
                    if !uncertainty_ids.contains(uncertainty) {
                        return Err(ExplorationContextError::UnresolvedImpactUncertainty {
                            reference: uncertainty.clone(),
                        });
                    }
                }
                ImpactEntry::Unaffected { .. } | ImpactEntry::NotApplicable { .. } => {}
            }
        }
        Ok(Self(raw))
    }

    pub fn evidence(&self) -> &[Evidence] {
        &self.0.evidence
    }

    pub fn claims(&self) -> &[Claim] {
        &self.0.claims
    }

    pub fn uncertainties(&self) -> &[UnresolvedFact] {
        &self.0.uncertainties
    }

    pub fn impact(&self) -> &[ImpactAssessment] {
        &self.0.impact
    }

    pub fn coverage(&self) -> &[ImpactCoverage] {
        &self.0.coverage
    }
}

/// A raw candidate that is not coherent, located by a JSON pointer into the
/// candidate document (relative to the candidate root).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} (at {pointer})")]
pub struct CandidateError {
    pointer: String,
    message: String,
}

impl CandidateError {
    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn at(pointer: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            pointer: pointer.into(),
            message: message.into(),
        }
    }

    fn from_delivery(error: &DeliveryError) -> Self {
        Self::at("/delivery/mrs", error.to_string())
    }
}

/// A plan candidate, proven coherent (design §6.6). Built by [`Candidate::parse`]
/// (all six proofs) or, during state proving of an already-validated document, by
/// [`Candidate::from_raw`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    raw: RawCandidate,
    regions: BTreeSet<PlanRegion>,
    fingerprints: BTreeMap<PlanRegion, RegionFingerprint>,
}

impl Candidate {
    pub(crate) fn from_raw(raw: RawCandidate) -> Self {
        let fingerprints = fingerprints(&raw);
        let regions = fingerprints.keys().cloned().collect();
        Self {
            raw,
            regions,
            fingerprints,
        }
    }

    pub const fn raw(&self) -> &RawCandidate {
        &self.raw
    }

    /// Every plan region this candidate carries.
    pub const fn regions(&self) -> &BTreeSet<PlanRegion> {
        &self.regions
    }

    /// The content digest of every region.
    pub const fn fingerprints(&self) -> &BTreeMap<PlanRegion, RegionFingerprint> {
        &self.fingerprints
    }

    /// Prove a raw candidate coherent, reporting the first failure with a JSON
    /// pointer relative to the candidate root (design §6.6).
    pub fn parse(raw: RawCandidate, repository: &Repository) -> Result<Self, CandidateError> {
        DeliveryGraph::parse(&raw.delivery).map_err(|e| CandidateError::from_delivery(&e))?;
        prove_coverage(&raw)?;
        prove_grounding(&raw, repository)?;
        prove_changed_symbols(&raw)?;
        prove_uncertainty_dispositions(&raw)?;
        Ok(Self::from_raw(raw))
    }
}

/// Prove the coverage checklist is total over `BlastLane::ALL`, each lane once;
/// every `Covered` lane names only declared impact subjects and at least one that
/// is not `NotApplicable` (design §6.6.2).
fn prove_coverage(raw: &RawCandidate) -> Result<(), CandidateError> {
    let subject_entries: BTreeMap<&NonEmptyText, &ImpactEntry> = raw
        .exploration
        .impact
        .iter()
        .map(|assessment| (&assessment.subject, &assessment.entry))
        .collect();
    let mut seen: BTreeSet<BlastLane> = BTreeSet::new();
    for (position, entry) in raw.exploration.coverage.iter().enumerate() {
        if !seen.insert(entry.lane) {
            return Err(CandidateError::at(
                format!("/exploration/coverage/{position}/lane"),
                format!("duplicate blast lane `{}`", entry.lane.label()),
            ));
        }
        if let LaneCoverage::Covered { subjects } = &entry.coverage {
            let mut any_applicable = false;
            for subject in subjects {
                match subject_entries.get(subject) {
                    None => {
                        return Err(CandidateError::at(
                            format!("/exploration/coverage/{position}"),
                            format!(
                                "coverage for lane `{}` names subject `{subject}`, which is not a declared impact subject",
                                entry.lane.label()
                            ),
                        ));
                    }
                    Some(entry) => {
                        if !matches!(entry, ImpactEntry::NotApplicable { .. }) {
                            any_applicable = true;
                        }
                    }
                }
            }
            if !any_applicable {
                return Err(CandidateError::at(
                    format!("/exploration/coverage/{position}"),
                    format!(
                        "covered lane `{}` names only not-applicable subjects",
                        entry.lane.label()
                    ),
                ));
            }
        }
    }
    for lane in BlastLane::ALL {
        if !seen.contains(&lane) {
            return Err(CandidateError::at(
                "/exploration/coverage",
                format!(
                    "blast lane `{}` is missing from the coverage checklist",
                    lane.label()
                ),
            ));
        }
    }
    Ok(())
}

/// Prove every evidence item that requires grounding carries a target equal to
/// the plan's repository identity (design §6.6.3).
fn prove_grounding(raw: &RawCandidate, repository: &Repository) -> Result<(), CandidateError> {
    for (position, evidence) in raw.exploration.evidence.iter().enumerate() {
        if !evidence.requires_grounding() {
            continue;
        }
        let pointer = format!("/exploration/evidence/{position}");
        match evidence.observed_target() {
            None => {
                return Err(CandidateError::at(
                    pointer,
                    format!(
                        "evidence `{}` has no target repository state; a candidate must ground every observation on its own repository identity",
                        evidence_id(evidence)
                    ),
                ));
            }
            Some(target) if !repository.identity_matches(target) => {
                return Err(CandidateError::at(
                    pointer,
                    format!(
                        "STALE-MODEL: evidence `{}` was observed against a repository other than the plan's (revision `{}`)",
                        evidence_id(evidence),
                        repository.revision,
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Prove every changed symbol's path appears in some `Mr.files`, and symbols are
/// unique (design §6.6.4).
fn prove_changed_symbols(raw: &RawCandidate) -> Result<(), CandidateError> {
    let files: BTreeSet<&str> = raw
        .delivery
        .mrs
        .iter()
        .flat_map(|mr| mr.files.iter())
        .map(crate::primitives::RepoRelPath::as_str)
        .collect();
    let mut symbols: BTreeSet<&NonEmptyText> = BTreeSet::new();
    for (position, changed) in raw.content.changed_symbols.iter().enumerate() {
        if !symbols.insert(&changed.symbol) {
            return Err(CandidateError::at(
                format!("/content/changed_symbols/{position}/symbol"),
                format!("duplicate changed symbol `{}`", changed.symbol),
            ));
        }
        if !files.contains(changed.path.as_str()) {
            return Err(CandidateError::at(
                format!("/content/changed_symbols/{position}/path"),
                format!(
                    "changed symbol `{}` lives in `{}`, which no merge request lists in its files",
                    changed.symbol,
                    changed.path.as_str()
                ),
            ));
        }
    }
    Ok(())
}

/// Prove every uncertainty disposition resolves: an `AcceptedByUser` names a
/// user-owned answered decision, a `ContainedByCriterion` names a declared
/// acceptance criterion (design §6.6.5).
fn prove_uncertainty_dispositions(raw: &RawCandidate) -> Result<(), CandidateError> {
    let user_answered: BTreeSet<&DecisionId> = raw
        .decisions
        .iter()
        .filter(|d| {
            matches!(d.owner, DecisionOwner::User)
                && matches!(d.state, DecisionState::Answered { .. })
        })
        .map(|d| &d.id)
        .collect();
    let criteria: BTreeSet<_> = raw
        .delivery
        .acceptance_criteria
        .iter()
        .map(|c| &c.id)
        .collect();
    for (position, uncertainty) in raw.exploration.uncertainties.iter().enumerate() {
        match &uncertainty.disposition {
            UncertaintyDisposition::AcceptedByUser { decision } => {
                if !user_answered.contains(decision) {
                    return Err(CandidateError::at(
                        format!("/exploration/uncertainties/{position}/disposition/decision"),
                        format!(
                            "uncertainty decision `{decision}` does not resolve to a user-owned answered decision"
                        ),
                    ));
                }
            }
            UncertaintyDisposition::ContainedByCriterion { criterion } => {
                if !criteria.contains(criterion) {
                    return Err(CandidateError::at(
                        format!("/exploration/uncertainties/{position}/disposition/criterion"),
                        format!("uncertainty criterion `{criterion}` does not resolve"),
                    ));
                }
            }
            UncertaintyDisposition::Open { .. } => {}
        }
    }
    Ok(())
}

/// The digest a [`crate::capability::CapabilityDecision`] binds to: the SHA-256 of
/// the report's canonical serialization.
pub fn report_digest(report: &CapabilityReport) -> Sha256Digest {
    Sha256Digest::hash(&serde_json::to_vec(report).unwrap_or_default())
}

/// The content fingerprint of a candidate: the SHA-256 of its canonical JSON.
/// Review packets key on this so sibling review keys stay valid across one apply
/// (design decision 7).
pub fn candidate_fingerprint(candidate: &Candidate) -> Sha256Digest {
    Sha256Digest::hash(&serde_json::to_vec(candidate.raw()).unwrap_or_default())
}

/// The proven run-state sum. Constructed only by parsing or workflow transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunState {
    AwaitingCapabilityApproval {
        report: CapabilityReport,
        prior: Option<ExplorationContext>,
    },
    Exploring {
        capabilities: ApprovedCapabilities,
        progress: ExplorationProgress,
    },
    AwaitingUserDecision {
        capabilities: ApprovedCapabilities,
        context: ExplorationContext,
        brief: DesignBrief,
        decisions: OneOrMore<ProposedDecision>,
        questions: OneOrMore<UserQuestion>,
    },
    Drafting {
        capabilities: ApprovedCapabilities,
        context: ExplorationContext,
        brief: DesignBrief,
        decisions: Vec<Decision>,
    },
    Reviewing {
        candidate: Candidate,
        wave: WaveNumber,
        pending: OneOrMore<ReviewKind>,
        completed: Vec<CompletedReview>,
        history: Vec<ReviewRound>,
    },
    Correcting {
        candidate: Candidate,
        wave: WaveNumber,
        reviews: OneOrMore<CompletedReview>,
        must_fix: Vec<FindingId>,
        history: Vec<ReviewRound>,
    },
    AwaitingReviewDecision {
        candidate: Candidate,
        wave: WaveNumber,
        unresolved: OneOrMore<Finding>,
        cause: ReviewEscalation,
        history: Vec<ReviewRound>,
    },
    Final {
        candidate: Candidate,
        history: Vec<ReviewRound>,
        accepted: Vec<Finding>,
        comments: Vec<Comment>,
    },
}

/// Proven exploration progress. Built only by parsing or workflow transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorationProgress {
    Collecting { prior: ExplorationContext },
    Complete { context: ExplorationContext },
}

impl ExplorationProgress {
    /// The starting exploration progress: collecting over an empty context.
    pub(crate) const fn initial() -> Self {
        Self::Collecting {
            prior: ExplorationContext::empty(),
        }
    }

    /// Resume collecting with the given context carried forward intact.
    pub(crate) const fn resuming(prior: ExplorationContext) -> Self {
        Self::Collecting { prior }
    }

    /// The context this progress carries, regardless of phase.
    pub const fn context(&self) -> &ExplorationContext {
        match self {
            Self::Collecting { prior } => prior,
            Self::Complete { context } => context,
        }
    }

    pub(crate) fn from_raw(raw: RawExplorationProgress) -> Self {
        match raw {
            RawExplorationProgress::Collecting { prior } => Self::Collecting {
                prior: ExplorationContext::from_raw(prior),
            },
            RawExplorationProgress::Complete { context } => Self::Complete {
                context: ExplorationContext::from_raw(context),
            },
        }
    }

    pub(crate) fn into_raw(self) -> RawExplorationProgress {
        match self {
            Self::Collecting { prior } => RawExplorationProgress::Collecting {
                prior: prior.into_raw(),
            },
            Self::Complete { context } => RawExplorationProgress::Complete {
                context: context.into_raw(),
            },
        }
    }
}

impl RawRunState {
    /// The exploration context carried by this state, if any, paired with the JSON
    /// pointer at which it sits under `/run`.
    pub(crate) const fn exploration(&self) -> Option<(&RawExplorationContext, &'static str)> {
        match self {
            Self::AwaitingCapabilityApproval { prior, .. } => match prior {
                Some(prior) => Some((prior, "/run/data/prior")),
                None => None,
            },
            Self::Exploring { progress, .. } => match progress {
                RawExplorationProgress::Collecting { prior } => {
                    Some((prior, "/run/data/progress/prior"))
                }
                RawExplorationProgress::Complete { context } => {
                    Some((context, "/run/data/progress/context"))
                }
            },
            Self::AwaitingUserDecision { context, .. } | Self::Drafting { context, .. } => {
                Some((context, "/run/data/context"))
            }
            Self::Reviewing { candidate, .. }
            | Self::Correcting { candidate, .. }
            | Self::AwaitingReviewDecision { candidate, .. }
            | Self::Final { candidate, .. } => {
                Some((&candidate.exploration, "/run/data/candidate/exploration"))
            }
        }
    }

    /// The candidate carried by this state, if any, paired with the JSON pointer
    /// at which it sits.
    pub(crate) const fn candidate(&self) -> Option<(&RawCandidate, &'static str)> {
        match self {
            Self::Reviewing { candidate, .. }
            | Self::Correcting { candidate, .. }
            | Self::AwaitingReviewDecision { candidate, .. }
            | Self::Final { candidate, .. } => Some((candidate, "/run/data/candidate")),
            Self::AwaitingCapabilityApproval { .. }
            | Self::Exploring { .. }
            | Self::AwaitingUserDecision { .. }
            | Self::Drafting { .. } => None,
        }
    }

    /// The delivery plan carried by this state, if any, with its JSON pointer.
    pub(crate) fn delivery(&self) -> Option<(&DeliveryPlan, String)> {
        self.candidate()
            .map(|(candidate, base)| (&candidate.delivery, format!("{base}/delivery")))
    }

    /// The accepted findings and history a final state carries, for the
    /// `accepted ⊆ history` load proof.
    pub(crate) fn final_accepted_and_history(&self) -> Option<(&[Finding], &[ReviewRound])> {
        match self {
            Self::Final {
                accepted, history, ..
            } => Some((accepted, history)),
            _ => None,
        }
    }

    /// Wrap validated raw state into proven state. Total: validation happened at
    /// parse.
    pub(crate) fn prove(self) -> RunState {
        match self {
            Self::AwaitingCapabilityApproval { report, prior } => {
                RunState::AwaitingCapabilityApproval {
                    report,
                    prior: prior.map(ExplorationContext::from_raw),
                }
            }
            Self::Exploring {
                capabilities,
                progress,
            } => RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::from_raw(progress),
            },
            Self::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                questions,
            } => RunState::AwaitingUserDecision {
                capabilities,
                context: ExplorationContext::from_raw(context),
                brief,
                decisions,
                questions,
            },
            Self::Drafting {
                capabilities,
                context,
                brief,
                decisions,
            } => RunState::Drafting {
                capabilities,
                context: ExplorationContext::from_raw(context),
                brief,
                decisions,
            },
            Self::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
            } => RunState::Reviewing {
                candidate: Candidate::from_raw(candidate),
                wave,
                pending,
                completed,
                history,
            },
            Self::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
            } => RunState::Correcting {
                candidate: Candidate::from_raw(candidate),
                wave,
                reviews,
                must_fix,
                history,
            },
            Self::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                cause,
                history,
            } => RunState::AwaitingReviewDecision {
                candidate: Candidate::from_raw(candidate),
                wave,
                unresolved,
                cause,
                history,
            },
            Self::Final {
                candidate,
                history,
                accepted,
                comments,
            } => RunState::Final {
                candidate: Candidate::from_raw(candidate),
                history,
                accepted,
                comments,
            },
        }
    }
}

impl RunState {
    /// The review comments overlaid on a final plan, in order. Empty in every
    /// non-final state.
    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        match self {
            Self::Final { comments, .. } => comments,
            _ => &[],
        }
    }

    /// The candidate this state carries, if any — the render/serve surface.
    #[must_use]
    pub const fn candidate(&self) -> Option<&Candidate> {
        match self {
            Self::Reviewing { candidate, .. }
            | Self::Correcting { candidate, .. }
            | Self::AwaitingReviewDecision { candidate, .. }
            | Self::Final { candidate, .. } => Some(candidate),
            Self::AwaitingCapabilityApproval { .. }
            | Self::Exploring { .. }
            | Self::AwaitingUserDecision { .. }
            | Self::Drafting { .. } => None,
        }
    }

    /// The set of every region the state's candidate carries. Empty when there is
    /// no candidate.
    #[must_use]
    pub fn regions(&self) -> BTreeSet<PlanRegion> {
        self.candidate()
            .map_or_else(BTreeSet::new, |candidate| candidate.regions().clone())
    }

    /// Project proven state back into raw state for canonical serialization.
    pub(crate) fn to_raw(&self) -> RawRunState {
        match self {
            Self::AwaitingCapabilityApproval { report, prior } => {
                RawRunState::AwaitingCapabilityApproval {
                    report: report.clone(),
                    prior: prior.as_ref().map(|p| p.raw().clone()),
                }
            }
            Self::Exploring {
                capabilities,
                progress,
            } => RawRunState::Exploring {
                capabilities: capabilities.clone(),
                progress: progress.clone().into_raw(),
            },
            Self::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                questions,
            } => RawRunState::AwaitingUserDecision {
                capabilities: capabilities.clone(),
                context: context.raw().clone(),
                brief: brief.clone(),
                decisions: decisions.clone(),
                questions: questions.clone(),
            },
            Self::Drafting {
                capabilities,
                context,
                brief,
                decisions,
            } => RawRunState::Drafting {
                capabilities: capabilities.clone(),
                context: context.raw().clone(),
                brief: brief.clone(),
                decisions: decisions.clone(),
            },
            Self::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
            } => RawRunState::Reviewing {
                candidate: candidate.raw().clone(),
                wave: *wave,
                pending: pending.clone(),
                completed: completed.clone(),
                history: history.clone(),
            },
            Self::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
            } => RawRunState::Correcting {
                candidate: candidate.raw().clone(),
                wave: *wave,
                reviews: reviews.clone(),
                must_fix: must_fix.clone(),
                history: history.clone(),
            },
            Self::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                cause,
                history,
            } => RawRunState::AwaitingReviewDecision {
                candidate: candidate.raw().clone(),
                wave: *wave,
                unresolved: unresolved.clone(),
                cause: cause.clone(),
                history: history.clone(),
            },
            Self::Final {
                candidate,
                history,
                accepted,
                comments,
            } => RawRunState::Final {
                candidate: candidate.raw().clone(),
                history: history.clone(),
                accepted: accepted.clone(),
                comments: comments.clone(),
            },
        }
    }
}

/// Every finding id recorded across a review history — the id-space an
/// `accepted` finding must belong to (design §12 load proof).
pub(crate) fn history_finding_ids(history: &[ReviewRound]) -> BTreeSet<FindingId> {
    history
        .iter()
        .flat_map(|round| round.reviews.iter())
        .flat_map(CompletedReview::findings)
        .map(|finding| finding.id.clone())
        .collect()
}
