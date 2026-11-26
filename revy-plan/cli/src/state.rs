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

use crate::capability::{CapabilityReport, RepositoryIdentity};
use crate::evidence::Evidence;
use crate::ids::{AcId, ClaimId, DecisionId, EvidenceId, FindingId, ProposalId, UncertaintyId};
use crate::issues::{Collector, Issues, JsonPointer};
use crate::model::claim::{Claim, UncertaintyDisposition, UnresolvedFact};
use crate::model::content::Block;
use crate::model::content::{Decision, DecisionOwner, DecisionState, DesignBrief, PlanContent};
use crate::model::content::{ProposedDecision, UserAnswer, UserQuestion};
use crate::model::delivery::{DeliveryError, DeliveryGraph, DeliveryPlan, Verify};
use crate::model::diagram::Diagram;
use crate::model::discussion::Comment;
use crate::model::impact::{
    BlastLane, ImpactAssessment, ImpactCoverage, ImpactEntry, LaneCoverage,
};
use crate::model::review::{
    CompletedReview, Finding, PlanRegion, RegionFingerprint, ReviewEscalation, ReviewKind, Ruling,
    Triage, fingerprints,
};
use crate::primitives::{NonEmptyText, OneOrMore, RepoAlias, RepoPath, Sha256Digest};
use crate::repository::{CitationError, Repo, Repositories};

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

    /// Whether this is the last automatic wave counted from `base` (the first
    /// wave, or the latest revision's): a material wave here escalates to the
    /// user rather than running another automatic correction.
    pub const fn is_budget_exhausted_since(self, base: Self) -> bool {
        self.0.get().saturating_sub(base.0.get()) + 1 >= Self::MAX_AUTOMATIC
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
    /// Who approved: the user, or the CLI because the report left nothing to
    /// decide. Absent means `user`.
    #[serde(default, skip_serializing_if = "Approval::is_user")]
    approval: Approval,
}

/// How a capability set came to be approved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Approval {
    /// The user decided over the report.
    #[default]
    User,
    /// The report proposed nothing and already had code intelligence, so the
    /// CLI proceeded without asking.
    Auto,
}

impl Approval {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` passes a reference
    const fn is_user(&self) -> bool {
        matches!(self, Self::User)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Auto => "auto",
        }
    }
}

/// An approved-capabilities set that names a proposal the report never disclosed,
/// or claims an automatic approval its report does not admit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovedCapabilityError {
    #[error("approval names undisclosed proposal `{proposal}`")]
    UndisclosedProposal { proposal: ProposalId },
    #[error(
        "automatic approval requires a report with no proposals and a capability proving `semantic` evidence"
    )]
    AutoApprovalNotAdmissible,
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
        Ok(Self {
            report,
            approved,
            approval: Approval::User,
        })
    }

    /// The approval the CLI records itself for a report that leaves the user
    /// nothing to decide; `None` when the user must decide.
    pub fn automatic(report: &CapabilityReport) -> Option<Self> {
        report.needs_no_approval().then(|| Self {
            report: report.clone(),
            approved: Vec::new(),
            approval: Approval::Auto,
        })
    }

    pub const fn approval(&self) -> Approval {
        self.approval
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
            #[serde(default)]
            approval: Approval,
        }
        let raw = Raw::deserialize(deserializer)?;
        match raw.approval {
            Approval::User => {
                Self::parse(raw.report, raw.approved).map_err(serde::de::Error::custom)
            }
            Approval::Auto if raw.approved.is_empty() => {
                Self::automatic(&raw.report).ok_or_else(|| {
                    serde::de::Error::custom(ApprovedCapabilityError::AutoApprovalNotAdmissible)
                })
            }
            Approval::Auto => Err(serde::de::Error::custom(
                ApprovedCapabilityError::AutoApprovalNotAdmissible,
            )),
        }
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
    /// Approved tools that were tried and failed; each one discharges that
    /// proposal's approved-tool-usage obligation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_failures: Vec<ToolFailure>,
}

/// An approved tool the explorer tried and could not use, and why — recorded in
/// place of the evidence the tool would have produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolFailure {
    /// The approved proposal id of the tool that failed.
    pub proposal: ProposalId,
    pub reason: NonEmptyText,
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

/// Which part of the run a reopen sends the plan back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReopenTarget {
    /// Back to collecting exploration evidence, capabilities kept.
    Exploration,
    /// Back to drafting over the same brief, decisions, and exploration.
    Draft,
}

impl ReopenTarget {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exploration => "exploration",
            Self::Draft => "draft",
        }
    }
}

/// One reopen: where the plan went back to, why, and the review record of the
/// candidate it set aside (kept for the render timeline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reopen {
    pub target: ReopenTarget,
    pub reason: NonEmptyText,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ReviewRound>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revisions: Vec<Revision>,
    /// The review comments a final plan carried when it was reopened.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
}

/// A user/orchestrator revision of a candidate under review.
///
/// The wave the revised candidate is reviewed in, what changed and why, the findings it
/// addresses, and the comments a final plan carried when it was revised. A
/// revision of an escalated plan records the escalation `cause`; it keeps the
/// automatic-wave budget counting from where it stood, so it cannot replace
/// the user's ruling with fresh automatic waves. `carried_accepted` holds the
/// findings a final plan accepted that this revision did not address: they are
/// accepted again at the next final. `decision_changes` lists every change to a
/// user-owned decision's answer or alternatives, for the user to see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub wave: WaveNumber,
    pub note: NonEmptyText,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<ReviewEscalation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carried_accepted: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_changes: Vec<DecisionDiff>,
}

/// One change a revision made to a user-owned decision.
///
/// A decision added or removed, its answer changed, or its set of alternatives
/// (the answer plus the ruled-out alternatives) changed. Alternative sets list
/// in sorted order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "field", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecisionDiff {
    Added {
        decision: DecisionId,
        after: Vec<NonEmptyText>,
    },
    Removed {
        decision: DecisionId,
        before: Vec<NonEmptyText>,
    },
    Answer {
        decision: DecisionId,
        before: Option<NonEmptyText>,
        after: Option<NonEmptyText>,
    },
    Alternatives {
        decision: DecisionId,
        before: Vec<NonEmptyText>,
        after: Vec<NonEmptyText>,
    },
}

impl DecisionDiff {
    pub const fn decision(&self) -> &DecisionId {
        match self {
            Self::Added { decision, .. }
            | Self::Removed { decision, .. }
            | Self::Answer { decision, .. }
            | Self::Alternatives { decision, .. } => decision,
        }
    }
}

/// The content and delivery of a draft set aside by a reopen — the subject a
/// `draft` packet `--patch` applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PriorDraft {
    pub content: PlanContent,
    pub delivery: DeliveryPlan,
}

/// The design a reopen set aside.
///
/// The brief and decisions (as proposed, with the user's answers), and the draft
/// when one existed. A `design_brief` packet
/// patches against the brief and decisions; a user decision whose question and
/// alternatives come back unchanged keeps its prior answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PriorDesign {
    pub brief: DesignBrief,
    pub decisions: Vec<ProposedDecision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub answers: Vec<UserAnswer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<PriorDraft>,
}

/// What a pre-candidate state carries across reopens: every reopen so far and
/// the design the latest one set aside (until a new draft replaces it).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignLineage {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reopens: Vec<Reopen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prior_design: Option<PriorDesign>,
}

impl DesignLineage {
    pub const fn is_empty(&self) -> bool {
        self.reopens.is_empty() && self.prior_design.is_none()
    }
}

/// What a candidate-bearing state carries beside its candidate.
///
/// The approved capabilities (absent on plans written before they were kept here, which
/// therefore cannot reopen), every reopen so far, and the revisions of this
/// candidate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewLineage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ApprovedCapabilities>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reopens: Vec<Reopen>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revisions: Vec<Revision>,
}

impl ReviewLineage {
    pub const fn is_empty(&self) -> bool {
        self.capabilities.is_none() && self.reopens.is_empty() && self.revisions.is_empty()
    }

    /// The wave the automatic budget counts from: the wave of the latest
    /// revision that did not answer an escalation, or the first.
    pub fn budget_base(&self) -> WaveNumber {
        self.revisions
            .iter()
            .rev()
            .find(|revision| revision.cause.is_none())
            .map_or_else(WaveNumber::first, |revision| revision.wave)
    }

    /// The accepted findings the latest revision carried forward, which the
    /// next final accepts again.
    pub fn carried_accepted(&self) -> &[Finding] {
        self.revisions
            .last()
            .map_or(&[], |revision| revision.carried_accepted.as_slice())
    }
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
        /// The user's instruction from a `request_revision` decision, until a
        /// revised report answers it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision_request: Option<NonEmptyText>,
        #[serde(default, skip_serializing_if = "DesignLineage::is_empty")]
        lineage: DesignLineage,
    },
    Exploring {
        capabilities: ApprovedCapabilities,
        progress: RawExplorationProgress,
        #[serde(default, skip_serializing_if = "DesignLineage::is_empty")]
        lineage: DesignLineage,
    },
    AwaitingUserDecision {
        capabilities: ApprovedCapabilities,
        context: RawExplorationContext,
        brief: DesignBrief,
        decisions: OneOrMore<ProposedDecision>,
        questions: OneOrMore<UserQuestion>,
        /// Answers kept from the prior design for decisions that came back
        /// unchanged; `questions` holds only the rest.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        carried_answers: Vec<UserAnswer>,
        #[serde(default, skip_serializing_if = "DesignLineage::is_empty")]
        lineage: DesignLineage,
    },
    Drafting {
        capabilities: ApprovedCapabilities,
        context: RawExplorationContext,
        brief: DesignBrief,
        decisions: Vec<Decision>,
        #[serde(default, skip_serializing_if = "DesignLineage::is_empty")]
        lineage: DesignLineage,
    },
    Reviewing {
        candidate: RawCandidate,
        wave: WaveNumber,
        pending: OneOrMore<ReviewKind>,
        #[serde(default)]
        completed: Vec<CompletedReview>,
        #[serde(default)]
        history: Vec<ReviewRound>,
        #[serde(default, skip_serializing_if = "ReviewLineage::is_empty")]
        lineage: ReviewLineage,
    },
    Correcting {
        candidate: RawCandidate,
        wave: WaveNumber,
        reviews: OneOrMore<CompletedReview>,
        #[serde(default)]
        must_fix: Vec<FindingId>,
        #[serde(default)]
        history: Vec<ReviewRound>,
        #[serde(default, skip_serializing_if = "ReviewLineage::is_empty")]
        lineage: ReviewLineage,
    },
    AwaitingReviewDecision {
        candidate: RawCandidate,
        wave: WaveNumber,
        unresolved: OneOrMore<Finding>,
        cause: ReviewEscalation,
        #[serde(default)]
        history: Vec<ReviewRound>,
        #[serde(default, skip_serializing_if = "ReviewLineage::is_empty")]
        lineage: ReviewLineage,
    },
    Final {
        candidate: RawCandidate,
        #[serde(default)]
        history: Vec<ReviewRound>,
        #[serde(default)]
        accepted: Vec<Finding>,
        #[serde(default)]
        comments: Vec<Comment>,
        #[serde(default, skip_serializing_if = "ReviewLineage::is_empty")]
        lineage: ReviewLineage,
    },
}

/// The stable id an evidence item carries, regardless of its kind.
pub(crate) const fn evidence_id(evidence: &Evidence) -> &EvidenceId {
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

/// A defect in an exploration context.
///
/// An unresolved cross-reference, a duplicate id, a dishonest sample, an
/// incomplete coverage checklist, or an observation not grounded on the plan's
/// repository identity.
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
    #[error("duplicate blast lane `{}`", lane.wire())]
    DuplicateLane { lane: BlastLane },
    #[error(
        "coverage for lane `{}` names subject `{subject}`, which is not a declared impact subject",
        lane.wire()
    )]
    UndeclaredCoverageSubject {
        lane: BlastLane,
        subject: NonEmptyText,
    },
    #[error("covered lane `{}` names only not-applicable subjects", lane.wire())]
    OnlyNotApplicableSubjects { lane: BlastLane },
    #[error(
        "blast lane(s) {} missing from the coverage checklist",
        lane_list(lanes)
    )]
    MissingLanes { lanes: Vec<BlastLane> },
    #[error(
        "tool failure names proposal `{proposal}`, which is not an approved proposal (approved: {})",
        if approved.is_empty() { "none".to_owned() } else { backticked(approved) }
    )]
    UnknownToolFailure {
        proposal: ProposalId,
        approved: Vec<ProposalId>,
    },
    #[error(
        "evidence `{id}` has no target repository state; every observation must be grounded on the identity of the repository it cites (`{repo}`, revision `{plan_revision}`)"
    )]
    Ungrounded {
        id: EvidenceId,
        repo: RepoAlias,
        plan_revision: NonEmptyText,
    },
    #[error(
        "STALE-MODEL: evidence `{id}` was observed against revision `{}` (dirty digest `{}`), but the plan's repository `{repo}` is at revision `{}` (dirty digest `{}`)",
        seen.revision, seen.dirty_digest, plan.revision, plan.dirty_digest
    )]
    StaleModel {
        id: EvidenceId,
        repo: RepoAlias,
        seen: RepositoryIdentity,
        plan: Box<RepositoryIdentity>,
    },
    #[error(
        "STALE-MODEL: evidence `{id}` cites no source anchor and was observed against revision `{}` (dirty digest `{}`), which is the state of none of the plan's repositories ({})",
        seen.revision, seen.dirty_digest, backticked(declared)
    )]
    UnmatchedTarget {
        id: EvidenceId,
        seen: RepositoryIdentity,
        declared: Vec<RepoAlias>,
    },
    #[error(
        "evidence `{id}` cites no source anchor and omits `target`; in a plan with several repositories set `target` to the identity of the repository it observed (one of {}; `revy-plan status` lists each identity)",
        backticked(declared)
    )]
    AnchorlessUntargeted {
        id: EvidenceId,
        declared: Vec<RepoAlias>,
    },
    #[error(
        "evidence `{id}` cites repositories {} but carries one `target`; split it into one evidence item per repository",
        backticked(repos)
    )]
    MixedRepositories {
        id: EvidenceId,
        repos: Vec<RepoAlias>,
    },
    #[error("evidence `{id}`: {error}")]
    UnresolvedRepository {
        id: EvidenceId,
        error: CitationError,
    },
}

fn backticked(items: &[impl std::fmt::Display]) -> String {
    items
        .iter()
        .map(|item| format!("`{item}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Which of the plan's repositories an evidence item was observed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CitedRepo<'a> {
    /// No anchor names a repository (documentation, runtime, history).
    Anchorless,
    /// Every anchor and scope names this one repository.
    One(Repo<'a>),
}

/// The repository an evidence item's anchors cite, or every reason they do
/// not cite exactly one declared repository. The one rule both grounding and
/// target stamping follow.
pub(crate) fn cited_repo<'a>(
    evidence: &Evidence,
    repositories: &'a Repositories,
) -> Result<CitedRepo<'a>, Vec<ExplorationContextError>> {
    let id = evidence_id(evidence);
    let mut resolved: Vec<Repo<'a>> = Vec::new();
    let mut errors = Vec::new();
    for alias in evidence.cited_repos() {
        match repositories.resolve(alias) {
            Ok(repo) => resolved.push(repo),
            Err(error) => errors.push(ExplorationContextError::UnresolvedRepository {
                id: id.clone(),
                error,
            }),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    match resolved.as_slice() {
        [] => Ok(CitedRepo::Anchorless),
        [repo] => Ok(CitedRepo::One(*repo)),
        repos => Err(vec![ExplorationContextError::MixedRepositories {
            id: id.clone(),
            repos: repos.iter().map(|repo| repo.alias.clone()).collect(),
        }]),
    }
}

fn lane_list(lanes: &[BlastLane]) -> String {
    lanes
        .iter()
        .map(|lane| format!("`{}`", lane.wire()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// How far exploration claims to be: still collecting, or complete — the
/// coverage checklist is owed only by a complete exploration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorationPhase {
    Collecting,
    Complete,
}

/// A proven exploration context. Built only by parsing or workflow transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplorationContext(RawExplorationContext);

type ContextIssues = Issues<ExplorationContextError>;

fn at(pointer: String) -> JsonPointer {
    JsonPointer::new(pointer)
}

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
            tool_failures: Vec::new(),
        })
    }

    /// Prove a raw context, reporting every defect with a pointer relative to the
    /// context root: unique ids, resolved cross-references, honest samples, tool
    /// failures naming approved proposals, every observation grounded on the
    /// plan's repository, and — once `phase` is complete — a total coverage
    /// checklist.
    pub(crate) fn parse(
        raw: RawExplorationContext,
        repositories: &Repositories,
        approved: &[ProposalId],
        phase: ExplorationPhase,
    ) -> Result<Self, ContextIssues> {
        let mut issues = Collector::new();
        prove_references(&raw, &mut issues);
        prove_tool_failures(&raw, approved, &mut issues);
        issues.absorb(prove_grounding(&raw, repositories), &JsonPointer::root());
        if phase == ExplorationPhase::Complete {
            issues.absorb(prove_coverage(&raw), &JsonPointer::root());
        }
        issues.finish(Self(raw))
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

    pub fn tool_failures(&self) -> &[ToolFailure] {
        &self.0.tool_failures
    }
}

/// Unique ids, resolved claim/impact references, and honest samples.
fn prove_references(raw: &RawExplorationContext, issues: &mut Collector<ExplorationContextError>) {
    let mut evidence_ids: BTreeSet<&EvidenceId> = BTreeSet::new();
    for (position, evidence) in raw.evidence.iter().enumerate() {
        let id = evidence_id(evidence);
        if !evidence_ids.insert(id) {
            issues.push(
                at(format!("/evidence/{position}/id")),
                ExplorationContextError::DuplicateEvidence { id: id.clone() },
            );
        }
        if !evidence.sample_admits() {
            issues.push(
                at(format!("/evidence/{position}")),
                ExplorationContextError::DishonestSample { id: id.clone() },
            );
        }
    }
    let mut claim_ids: BTreeSet<&ClaimId> = BTreeSet::new();
    for (position, claim) in raw.claims.iter().enumerate() {
        if !claim_ids.insert(&claim.id) {
            issues.push(
                at(format!("/claims/{position}/id")),
                ExplorationContextError::DuplicateClaim {
                    id: claim.id.clone(),
                },
            );
        }
        for (reference_position, reference) in claim.evidence.iter().enumerate() {
            if !evidence_ids.contains(reference) {
                issues.push(
                    at(format!("/claims/{position}/evidence/{reference_position}")),
                    ExplorationContextError::UnresolvedClaimEvidence {
                        reference: reference.clone(),
                    },
                );
            }
        }
    }
    let uncertainty_ids: BTreeSet<&UncertaintyId> =
        raw.uncertainties.iter().map(|u| &u.id).collect();
    let mut impact_subjects: BTreeSet<&NonEmptyText> = BTreeSet::new();
    for (position, assessment) in raw.impact.iter().enumerate() {
        if !impact_subjects.insert(&assessment.subject) {
            issues.push(
                at(format!("/impact/{position}/subject")),
                ExplorationContextError::DuplicateImpactSubject {
                    subject: assessment.subject.clone(),
                },
            );
        }
        match &assessment.entry {
            ImpactEntry::Affected { claims, .. } => {
                for (claim_position, reference) in claims.iter().enumerate() {
                    if !claim_ids.contains(reference) {
                        issues.push(
                            at(format!("/impact/{position}/entry/claims/{claim_position}")),
                            ExplorationContextError::UnresolvedImpactClaim {
                                reference: reference.clone(),
                            },
                        );
                    }
                }
            }
            ImpactEntry::Uncertain { uncertainty, .. } => {
                if !uncertainty_ids.contains(uncertainty) {
                    issues.push(
                        at(format!("/impact/{position}/entry/uncertainty")),
                        ExplorationContextError::UnresolvedImpactUncertainty {
                            reference: uncertainty.clone(),
                        },
                    );
                }
            }
            ImpactEntry::Unaffected { .. } | ImpactEntry::NotApplicable { .. } => {}
        }
    }
}

/// Every tool failure in a candidate's exploration names a proposal the user
/// approved — the exploration check, rerun when a correction or revision
/// replaces the candidate.
pub(crate) fn prove_tool_failures_approved(
    raw: &RawExplorationContext,
    approved: &[ProposalId],
) -> Result<(), ContextIssues> {
    let mut issues = Collector::new();
    prove_tool_failures(raw, approved, &mut issues);
    issues.finish(())
}

/// Every tool failure names a proposal the user approved.
fn prove_tool_failures(
    raw: &RawExplorationContext,
    approved: &[ProposalId],
    issues: &mut Collector<ExplorationContextError>,
) {
    for (position, failure) in raw.tool_failures.iter().enumerate() {
        if !approved.contains(&failure.proposal) {
            issues.push(
                at(format!("/tool_failures/{position}/proposal")),
                ExplorationContextError::UnknownToolFailure {
                    proposal: failure.proposal.clone(),
                    approved: approved.to_vec(),
                },
            );
        }
    }
}

/// Prove the coverage checklist is total over `BlastLane::ALL`, each lane once;
/// every `Covered` lane names only declared impact subjects and at least one that
/// is not `NotApplicable` (design §6.6.2). Pointers are relative to the
/// exploration context root.
pub(crate) fn prove_coverage(raw: &RawExplorationContext) -> Result<(), ContextIssues> {
    let mut issues = Collector::new();
    let subject_entries: BTreeMap<&NonEmptyText, &ImpactEntry> = raw
        .impact
        .iter()
        .map(|assessment| (&assessment.subject, &assessment.entry))
        .collect();
    let mut seen: BTreeSet<BlastLane> = BTreeSet::new();
    for (position, entry) in raw.coverage.iter().enumerate() {
        if !seen.insert(entry.lane) {
            issues.push(
                at(format!("/coverage/{position}/lane")),
                ExplorationContextError::DuplicateLane { lane: entry.lane },
            );
        }
        if let LaneCoverage::Covered { subjects } = &entry.coverage {
            let mut any_applicable = false;
            for subject in subjects {
                match subject_entries.get(subject) {
                    None => issues.push(
                        at(format!("/coverage/{position}")),
                        ExplorationContextError::UndeclaredCoverageSubject {
                            lane: entry.lane,
                            subject: subject.clone(),
                        },
                    ),
                    Some(entry) => {
                        if !matches!(entry, ImpactEntry::NotApplicable { .. }) {
                            any_applicable = true;
                        }
                    }
                }
            }
            let any_undeclared = subjects
                .iter()
                .any(|subject| !subject_entries.contains_key(subject));
            if !any_applicable && !any_undeclared {
                issues.push(
                    at(format!("/coverage/{position}")),
                    ExplorationContextError::OnlyNotApplicableSubjects { lane: entry.lane },
                );
            }
        }
    }
    let missing: Vec<BlastLane> = BlastLane::ALL
        .into_iter()
        .filter(|lane| !seen.contains(lane))
        .collect();
    if !missing.is_empty() {
        issues.push(
            at("/coverage".to_owned()),
            ExplorationContextError::MissingLanes { lanes: missing },
        );
    }
    issues.finish(())
}

/// Prove every evidence item that requires grounding carries a target equal to
/// the plan's repository identity (design §6.6.3). Pointers are relative to the
/// exploration context root.
pub(crate) fn prove_grounding(
    raw: &RawExplorationContext,
    repositories: &Repositories,
) -> Result<(), ContextIssues> {
    let mut issues = Collector::new();
    for (position, evidence) in raw.evidence.iter().enumerate() {
        if !evidence.requires_grounding() {
            continue;
        }
        let pointer = at(format!("/evidence/{position}"));
        let cited = match cited_repo(evidence, repositories) {
            Ok(cited) => cited,
            Err(errors) => {
                for error in errors {
                    issues.push(pointer.clone(), error);
                }
                continue;
            }
        };
        if let Some(error) = grounding_error(evidence, cited, repositories) {
            issues.push(pointer, error);
        }
    }
    issues.finish(())
}

/// Why `evidence`'s target is not the identity of the repository it cites —
/// for anchorless evidence, of any declared repository.
fn grounding_error(
    evidence: &Evidence,
    cited: CitedRepo<'_>,
    repositories: &Repositories,
) -> Option<ExplorationContextError> {
    let id = evidence_id(evidence).clone();
    let expected = match cited {
        CitedRepo::One(repo) => repo,
        CitedRepo::Anchorless => repositories.primary_repo(),
    };
    let Some(target) = evidence.observed_target() else {
        if matches!(cited, CitedRepo::Anchorless) && !repositories.others().is_empty() {
            return Some(ExplorationContextError::AnchorlessUntargeted {
                id,
                declared: repositories.aliases(),
            });
        }
        return Some(ExplorationContextError::Ungrounded {
            id,
            repo: expected.alias.clone(),
            plan_revision: expected.repository.revision.clone(),
        });
    };
    let matches = match cited {
        CitedRepo::One(repo) => repo.repository.identity_matches(target),
        CitedRepo::Anchorless => repositories
            .iter()
            .any(|repo| repo.repository.identity_matches(target)),
    };
    if matches {
        return None;
    }
    if matches!(cited, CitedRepo::Anchorless) && !repositories.others().is_empty() {
        return Some(ExplorationContextError::UnmatchedTarget {
            id,
            seen: target.clone(),
            declared: repositories.aliases(),
        });
    }
    Some(ExplorationContextError::StaleModel {
        id,
        repo: expected.alias.clone(),
        seen: target.clone(),
        plan: Box::new(expected.repository.identity()),
    })
}

/// A raw candidate that is not coherent (design §6.6).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CandidateError {
    #[error(transparent)]
    Exploration(#[from] ExplorationContextError),
    #[error(transparent)]
    Delivery(#[from] DeliveryError),
    #[error("duplicate changed symbol `{symbol}`")]
    DuplicateChangedSymbol { symbol: NonEmptyText },
    #[error(
        "changed symbol `{symbol}` lives in `{path}`, which no merge request lists in its files"
    )]
    UnlistedChangedSymbol {
        symbol: NonEmptyText,
        path: RepoPath,
    },
    #[error("`{path}`: {error}")]
    UnresolvedRepository {
        path: RepoPath,
        error: CitationError,
    },
    #[error("uncertainty decision `{decision}` does not resolve to a user-owned answered decision")]
    UnresolvedUserDecision { decision: DecisionId },
    #[error("uncertainty criterion `{criterion}` does not resolve")]
    UnresolvedCriterion { criterion: AcId },
}

/// Every defect a candidate carries, located relative to the candidate root.
pub type CandidateIssues = Issues<CandidateError>;

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

    /// Prove a raw candidate coherent, reporting every defect with a JSON pointer
    /// relative to the candidate root (design §6.6). Coverage and grounding are
    /// the same proofs a completing exploration runs.
    pub fn parse(raw: RawCandidate, repositories: &Repositories) -> Result<Self, CandidateIssues> {
        let mut issues = Collector::new();
        if let Err(error) = DeliveryGraph::parse(&raw.delivery) {
            issues.push(JsonPointer::new("/delivery/mrs"), error.into());
        }
        let exploration = JsonPointer::new("/exploration");
        issues.absorb(
            prove_coverage(&raw.exploration).map_err(|i| i.map(CandidateError::from)),
            &exploration,
        );
        issues.absorb(
            prove_grounding(&raw.exploration, repositories)
                .map_err(|i| i.map(CandidateError::from)),
            &exploration,
        );
        prove_changed_symbols(&raw, &mut issues);
        prove_cited_repositories(&raw, repositories, &mut issues);
        prove_uncertainty_dispositions(&raw, &mut issues);
        issues.finish(())?;
        Ok(Self::from_raw(raw))
    }
}

/// Prove every changed symbol's path appears in some `Mr.files`, and symbols are
/// unique (design §6.6.4).
fn prove_changed_symbols(raw: &RawCandidate, issues: &mut Collector<CandidateError>) {
    let files: BTreeSet<&RepoPath> = raw
        .delivery
        .mrs
        .iter()
        .flat_map(|mr| mr.files.iter())
        .collect();
    let mut symbols: BTreeSet<&NonEmptyText> = BTreeSet::new();
    for (position, changed) in raw.content.changed_symbols.iter().enumerate() {
        if !symbols.insert(&changed.symbol) {
            issues.push(
                at(format!("/content/changed_symbols/{position}/symbol")),
                CandidateError::DuplicateChangedSymbol {
                    symbol: changed.symbol.clone(),
                },
            );
        }
        if !files.contains(&changed.path) {
            issues.push(
                at(format!("/content/changed_symbols/{position}/path")),
                CandidateError::UnlistedChangedSymbol {
                    symbol: changed.symbol.clone(),
                    path: changed.path.clone(),
                },
            );
        }
    }
}

/// Prove every repository path a candidate authors — merge-request files,
/// changed-symbol paths, verify-command working directories, and tree-diagram
/// entries — names a declared repository, in its canonical
/// (primary-unprefixed) form.
fn prove_cited_repositories(
    raw: &RawCandidate,
    repositories: &Repositories,
    issues: &mut Collector<CandidateError>,
) {
    let files = raw.delivery.mrs.iter().enumerate().flat_map(|(mr, entry)| {
        entry
            .files
            .iter()
            .enumerate()
            .map(move |(file, path)| (format!("/delivery/mrs/{mr}/files/{file}"), path))
    });
    let symbols = raw
        .content
        .changed_symbols
        .iter()
        .enumerate()
        .map(|(position, changed)| {
            (
                format!("/content/changed_symbols/{position}/path"),
                &changed.path,
            )
        });
    let mr_cwds = raw
        .delivery
        .mrs
        .iter()
        .enumerate()
        .filter_map(|(mr, entry)| {
            entry
                .verify
                .as_ref()
                .map(|verify| (format!("/delivery/mrs/{mr}/verify/cwd"), &verify.cwd))
        });
    let criterion_cwds = raw
        .delivery
        .acceptance_criteria
        .iter()
        .enumerate()
        .filter_map(|(position, criterion)| match &criterion.verify {
            Verify::Command(command) => Some((
                format!("/delivery/acceptance_criteria/{position}/verify/cwd"),
                &command.cwd,
            )),
            Verify::Manual { .. } => None,
        });
    let tree_paths = raw
        .content
        .sections
        .iter()
        .enumerate()
        .flat_map(|(section, entry)| {
            entry
                .blocks
                .iter()
                .enumerate()
                .map(move |(block, content)| (section, block, content))
        })
        .flat_map(|(section, block, content)| match content {
            Block::Figure {
                diagram: Diagram::Tree { entries },
            } => entries
                .iter()
                .enumerate()
                .map(|(position, tree)| {
                    (
                        format!(
                            "/content/sections/{section}/blocks/{block}/diagram/entries/{position}/path"
                        ),
                        &tree.path,
                    )
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        });
    let paths = files
        .chain(symbols)
        .chain(mr_cwds)
        .chain(criterion_cwds)
        .chain(tree_paths);
    for (pointer, path) in paths {
        if let Err(error) = repositories.resolve(path.repo()) {
            issues.push(
                at(pointer),
                CandidateError::UnresolvedRepository {
                    path: path.clone(),
                    error,
                },
            );
        }
    }
}

/// Prove every uncertainty disposition resolves: an `AcceptedByUser` names a
/// user-owned answered decision, a `ContainedByCriterion` names a declared
/// acceptance criterion (design §6.6.5).
fn prove_uncertainty_dispositions(raw: &RawCandidate, issues: &mut Collector<CandidateError>) {
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
                    issues.push(
                        at(format!(
                            "/exploration/uncertainties/{position}/disposition/decision"
                        )),
                        CandidateError::UnresolvedUserDecision {
                            decision: decision.clone(),
                        },
                    );
                }
            }
            UncertaintyDisposition::ContainedByCriterion { criterion } => {
                if !criteria.contains(criterion) {
                    issues.push(
                        at(format!(
                            "/exploration/uncertainties/{position}/disposition/criterion"
                        )),
                        CandidateError::UnresolvedCriterion {
                            criterion: criterion.clone(),
                        },
                    );
                }
            }
            UncertaintyDisposition::Open { .. } => {}
        }
    }
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
        revision_request: Option<NonEmptyText>,
        lineage: DesignLineage,
    },
    Exploring {
        capabilities: ApprovedCapabilities,
        progress: ExplorationProgress,
        lineage: DesignLineage,
    },
    AwaitingUserDecision {
        capabilities: ApprovedCapabilities,
        context: ExplorationContext,
        brief: DesignBrief,
        decisions: OneOrMore<ProposedDecision>,
        questions: OneOrMore<UserQuestion>,
        carried_answers: Vec<UserAnswer>,
        lineage: DesignLineage,
    },
    Drafting {
        capabilities: ApprovedCapabilities,
        context: ExplorationContext,
        brief: DesignBrief,
        decisions: Vec<Decision>,
        lineage: DesignLineage,
    },
    Reviewing {
        candidate: Candidate,
        wave: WaveNumber,
        pending: OneOrMore<ReviewKind>,
        completed: Vec<CompletedReview>,
        history: Vec<ReviewRound>,
        lineage: ReviewLineage,
    },
    Correcting {
        candidate: Candidate,
        wave: WaveNumber,
        reviews: OneOrMore<CompletedReview>,
        must_fix: Vec<FindingId>,
        history: Vec<ReviewRound>,
        lineage: ReviewLineage,
    },
    AwaitingReviewDecision {
        candidate: Candidate,
        wave: WaveNumber,
        unresolved: OneOrMore<Finding>,
        cause: ReviewEscalation,
        history: Vec<ReviewRound>,
        lineage: ReviewLineage,
    },
    Final {
        candidate: Candidate,
        history: Vec<ReviewRound>,
        accepted: Vec<Finding>,
        comments: Vec<Comment>,
        lineage: ReviewLineage,
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
    // One arm per run-state variant, each a field-for-field move.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn prove(self) -> RunState {
        match self {
            Self::AwaitingCapabilityApproval {
                report,
                prior,
                revision_request,
                lineage,
            } => RunState::AwaitingCapabilityApproval {
                report,
                prior: prior.map(ExplorationContext::from_raw),
                revision_request,
                lineage,
            },
            Self::Exploring {
                capabilities,
                progress,
                lineage,
            } => RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::from_raw(progress),
                lineage,
            },
            Self::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            } => RunState::AwaitingUserDecision {
                capabilities,
                context: ExplorationContext::from_raw(context),
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            },
            Self::Drafting {
                capabilities,
                context,
                brief,
                decisions,
                lineage,
            } => RunState::Drafting {
                capabilities,
                context: ExplorationContext::from_raw(context),
                brief,
                decisions,
                lineage,
            },
            Self::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
                lineage,
            } => RunState::Reviewing {
                candidate: Candidate::from_raw(candidate),
                wave,
                pending,
                completed,
                history,
                lineage,
            },
            Self::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
                lineage,
            } => RunState::Correcting {
                candidate: Candidate::from_raw(candidate),
                wave,
                reviews,
                must_fix,
                history,
                lineage,
            },
            Self::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                cause,
                history,
                lineage,
            } => RunState::AwaitingReviewDecision {
                candidate: Candidate::from_raw(candidate),
                wave,
                unresolved,
                cause,
                history,
                lineage,
            },
            Self::Final {
                candidate,
                history,
                accepted,
                comments,
                lineage,
            } => RunState::Final {
                candidate: Candidate::from_raw(candidate),
                history,
                accepted,
                comments,
                lineage,
            },
        }
    }
}

impl RunState {
    /// The state a new plan opens in: awaiting the user's decision over its
    /// capability report, or — when the report leaves nothing to decide —
    /// exploring under an automatic approval. Only `init` approves
    /// automatically; every later report waits for the user, who either asked
    /// for it or must see what an exploration proposed.
    pub(crate) fn opening(report: CapabilityReport) -> Self {
        let Some(capabilities) = ApprovedCapabilities::automatic(&report) else {
            return Self::AwaitingCapabilityApproval {
                report,
                prior: None,
                revision_request: None,
                lineage: DesignLineage::default(),
            };
        };
        Self::Exploring {
            capabilities,
            progress: ExplorationProgress::initial(),
            lineage: DesignLineage::default(),
        }
    }

    /// The review comments overlaid on a final plan, in order. Empty in every
    /// non-final state.
    #[must_use]
    pub fn comments(&self) -> &[Comment] {
        match self {
            Self::Final { comments, .. } => comments,
            _ => &[],
        }
    }

    /// The review history of the current candidate. Empty before a candidate.
    #[must_use]
    pub fn history(&self) -> &[ReviewRound] {
        match self {
            Self::Reviewing { history, .. }
            | Self::Correcting { history, .. }
            | Self::AwaitingReviewDecision { history, .. }
            | Self::Final { history, .. } => history,
            _ => &[],
        }
    }

    /// The findings a final plan accepted unresolved. Empty in every other state.
    #[must_use]
    pub fn accepted(&self) -> &[Finding] {
        match self {
            Self::Final { accepted, .. } => accepted,
            _ => &[],
        }
    }

    /// Every reopen the run went through, oldest first.
    #[must_use]
    pub fn reopens(&self) -> &[Reopen] {
        match self {
            Self::AwaitingCapabilityApproval { lineage, .. }
            | Self::Exploring { lineage, .. }
            | Self::AwaitingUserDecision { lineage, .. }
            | Self::Drafting { lineage, .. } => &lineage.reopens,
            Self::Reviewing { lineage, .. }
            | Self::Correcting { lineage, .. }
            | Self::AwaitingReviewDecision { lineage, .. }
            | Self::Final { lineage, .. } => &lineage.reopens,
        }
    }

    /// The revisions of the current candidate, oldest first.
    #[must_use]
    pub fn revisions(&self) -> &[Revision] {
        match self {
            Self::Reviewing { lineage, .. }
            | Self::Correcting { lineage, .. }
            | Self::AwaitingReviewDecision { lineage, .. }
            | Self::Final { lineage, .. } => &lineage.revisions,
            _ => &[],
        }
    }

    /// The approved capability set, where the state records one.
    #[must_use]
    pub const fn capabilities(&self) -> Option<&ApprovedCapabilities> {
        match self {
            Self::AwaitingCapabilityApproval { .. } => None,
            Self::Exploring { capabilities, .. }
            | Self::AwaitingUserDecision { capabilities, .. }
            | Self::Drafting { capabilities, .. } => Some(capabilities),
            Self::Reviewing { lineage, .. }
            | Self::Correcting { lineage, .. }
            | Self::AwaitingReviewDecision { lineage, .. }
            | Self::Final { lineage, .. } => lineage.capabilities.as_ref(),
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
    // One arm per run-state variant, each a field-for-field move.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn to_raw(&self) -> RawRunState {
        match self.clone() {
            Self::AwaitingCapabilityApproval {
                report,
                prior,
                revision_request,
                lineage,
            } => RawRunState::AwaitingCapabilityApproval {
                report,
                prior: prior.map(ExplorationContext::into_raw),
                revision_request,
                lineage,
            },
            Self::Exploring {
                capabilities,
                progress,
                lineage,
            } => RawRunState::Exploring {
                capabilities,
                progress: progress.into_raw(),
                lineage,
            },
            Self::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            } => RawRunState::AwaitingUserDecision {
                capabilities,
                context: context.into_raw(),
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            },
            Self::Drafting {
                capabilities,
                context,
                brief,
                decisions,
                lineage,
            } => RawRunState::Drafting {
                capabilities,
                context: context.into_raw(),
                brief,
                decisions,
                lineage,
            },
            Self::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
                lineage,
            } => RawRunState::Reviewing {
                candidate: candidate.raw,
                wave,
                pending,
                completed,
                history,
                lineage,
            },
            Self::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
                lineage,
            } => RawRunState::Correcting {
                candidate: candidate.raw,
                wave,
                reviews,
                must_fix,
                history,
                lineage,
            },
            Self::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                cause,
                history,
                lineage,
            } => RawRunState::AwaitingReviewDecision {
                candidate: candidate.raw,
                wave,
                unresolved,
                cause,
                history,
                lineage,
            },
            Self::Final {
                candidate,
                history,
                accepted,
                comments,
                lineage,
            } => RawRunState::Final {
                candidate: candidate.raw,
                history,
                accepted,
                comments,
                lineage,
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
