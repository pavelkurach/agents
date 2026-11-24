//! Work packets and the results they consume.
//!
//! The closed vocabulary of work the frontier can emit, the raw external results
//! those packets consume, and the checked results the workflow accepts.
//! `RawWorkResult` is confined to this module and `runtime`; workflow transitions
//! receive only [`CheckedWorkResult`].

use schemars::{JsonSchema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capability::{CapabilityDecision, CapabilityReport};
use crate::evidence::Evidence;
use crate::model::claim::{Claim, UnresolvedFact};
use crate::model::content::{
    DesignBrief, PlanContent, ProposedDecision, SpecialistFocus, UserAnswer,
};
use crate::model::delivery::DeliveryPlan;
use crate::model::impact::{ImpactAssessment, ImpactCoverage};
use crate::model::review::{CompletedReview, Ruling, Triage};
use crate::primitives::{NonEmptyText, OneOrMore, Sha256Digest};
use crate::state::RawCandidate;

/// The closed set of work the frontier can dispatch. Exactly ten kinds (design
/// §18.5). The result each consumes is derived from this kind, never a parallel
/// list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkKind {
    CapabilityReport,
    CapabilityApproval,
    Exploration,
    DesignBrief,
    UserDecision,
    Draft,
    IntegratedReview,
    SpecialistReview { focus: SpecialistFocus },
    Correction,
    ReviewDecision,
}

impl WorkKind {
    /// Every packet kind, in canonical order (design §18.5).
    ///
    /// The single source both [`packet_kind_names`] and [`packet_schema_by_name`]
    /// derive from. The specialist focus is a placeholder: a specialist packet's
    /// result schema is the same [`CompletedReview`] regardless of focus. The
    /// derived `JsonSchema` couples the release-invariant count to this enum, so
    /// an eleventh variant fails the "exactly ten" test (see `work_kind_schema`).
    pub const ALL: [Self; 10] = [
        Self::CapabilityReport,
        Self::CapabilityApproval,
        Self::Exploration,
        Self::DesignBrief,
        Self::UserDecision,
        Self::Draft,
        Self::IntegratedReview,
        Self::SpecialistReview {
            focus: SpecialistFocus::Concurrency,
        },
        Self::Correction,
        Self::ReviewDecision,
    ];

    /// The serialized name of this kind — its `schema_ref` and the `--packet`
    /// argument to `revy plan schema`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CapabilityReport => "capability_report",
            Self::CapabilityApproval => "capability_approval",
            Self::Exploration => "exploration",
            Self::DesignBrief => "design_brief",
            Self::UserDecision => "user_decision",
            Self::Draft => "draft",
            Self::IntegratedReview => "integrated_review",
            Self::SpecialistReview { .. } => "specialist_review",
            Self::Correction => "correction",
            Self::ReviewDecision => "review_decision",
        }
    }
}

/// The subject a work packet is scoped to.
///
/// Combined with the [`WorkKind`] it yields a stable [`WorkKey`]. Review packets
/// key on the candidate fingerprint so sibling review keys stay valid across one
/// apply (design decision 7); other packets key on the run snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "subject", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkSubject {
    Report { digest: Sha256Digest },
    RunSnapshot { digest: Sha256Digest },
    Candidate { fingerprint: Sha256Digest },
}

/// A content-addressed identity for one ready packet: the digest of its kind and
/// subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct WorkKey(String);

impl WorkKey {
    pub(crate) fn derive(kind: WorkKind, subject: &WorkSubject) -> Self {
        let canonical = serde_json::to_vec(&(kind, subject)).unwrap_or_default();
        Self(format!("work-{}", Sha256Digest::hash(&canonical)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What exploration asks for next: to finish, or to add a capability before
/// continuing (proposing a fresh report the user must approve).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExplorationNext {
    Complete,
    ProposeCapability { report: CapabilityReport },
}

/// The bundle an exploration packet returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplorationResult {
    pub evidence: Vec<Evidence>,
    pub claims: Vec<Claim>,
    pub uncertainties: Vec<UnresolvedFact>,
    pub impact: Vec<ImpactAssessment>,
    #[serde(default)]
    pub coverage: Vec<ImpactCoverage>,
    pub next: ExplorationNext,
}

/// The brief an authoring packet returns: the problem framing plus the decisions
/// it proposes, each tagged with its owner (design §6.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignBriefResult {
    pub brief: DesignBrief,
    pub decisions: Vec<ProposedDecision>,
}

/// The user's decision over the open questions: answer them, revise the design, or
/// send exploration back for more evidence (design §6.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum UserDecisionResult {
    Answer { answers: OneOrMore<UserAnswer> },
    ReviseDesign { instruction: NonEmptyText },
    RequestEvidence { question: NonEmptyText },
}

/// The authored draft a `Draft` packet returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftResult {
    pub content: PlanContent,
    pub delivery: DeliveryPlan,
}

/// The re-authored plan a `Correction` packet returns.
///
/// The full corrected candidate plus the planner's triage of every material
/// finding (design §8.3). The candidate is boxed: it is much larger than any
/// other packet result, so carrying it inline would bloat `RawWorkResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CorrectionResult {
    pub candidate: Box<RawCandidate>,
    #[serde(default)]
    pub triage: Vec<Triage>,
}

/// The user's rulings over the escalated findings (design §8.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionResult {
    pub rulings: Vec<Ruling>,
}

/// A raw external result, decoded from packet bytes and confined to `packet` and
/// `runtime`. Workflow transitions receive only [`CheckedWorkResult`].
#[derive(Debug)]
pub enum RawWorkResult {
    CapabilityReport(CapabilityReport),
    CapabilityDecision(CapabilityDecision),
    Exploration(ExplorationResult),
    DesignBrief(DesignBriefResult),
    UserDecision(UserDecisionResult),
    Draft(DraftResult),
    Review(CompletedReview),
    Correction(CorrectionResult),
    ReviewDecision(ReviewDecisionResult),
}

/// A result whose filesystem-backed facts have been checked.
///
/// Evidence anchors, transcript digests, and approved-tool usage have been
/// verified by [`crate::runtime::check_result`]. Workflow transitions accept only
/// this, constructed only by `runtime`.
#[derive(Debug)]
pub struct CheckedWorkResult(RawWorkResult);

impl CheckedWorkResult {
    pub(crate) const fn checked(raw: RawWorkResult) -> Self {
        Self(raw)
    }

    pub(crate) fn into_raw(self) -> RawWorkResult {
        self.0
    }
}

/// A packet result that does not satisfy its packet schema.
#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("result does not satisfy the `{kind}` schema: {source}")]
    Malformed {
        kind: &'static str,
        #[source]
        source: serde_json::Error,
    },
}

// The single source mapping each [`WorkKind`] to its result type: both
// `decode_result` and the packet schema are derived from this list. The mapping
// is total, so the match is exhaustive and carries no wildcard.
macro_rules! packet_results {
    ($($kind:pat => $ty:ty => $variant:ident),+ $(,)?) => {
        fn decode(kind: WorkKind, bytes: &[u8]) -> Result<RawWorkResult, DecodeError> {
            match kind {
                $($kind =>
                    serde_json::from_slice::<$ty>(bytes)
                        .map(RawWorkResult::$variant)
                        .map_err(|source| DecodeError::Malformed { kind: kind.name(), source }),
                )+
            }
        }

        fn schema_for(kind: WorkKind) -> serde_json::Value {
            match kind {
                $($kind => result_schema::<$ty>(),)+
            }
        }
    };
}

packet_results!(
    WorkKind::CapabilityReport => CapabilityReport => CapabilityReport,
    WorkKind::CapabilityApproval => CapabilityDecision => CapabilityDecision,
    WorkKind::Exploration => ExplorationResult => Exploration,
    WorkKind::DesignBrief => DesignBriefResult => DesignBrief,
    WorkKind::UserDecision => UserDecisionResult => UserDecision,
    WorkKind::Draft => DraftResult => Draft,
    WorkKind::IntegratedReview => CompletedReview => Review,
    WorkKind::SpecialistReview { .. } => CompletedReview => Review,
    WorkKind::Correction => CorrectionResult => Correction,
    WorkKind::ReviewDecision => ReviewDecisionResult => ReviewDecision,
);

fn result_schema<T: JsonSchema>() -> serde_json::Value {
    SchemaGenerator::default()
        .into_root_schema_for::<T>()
        .into()
}

/// The result schema for one packet kind, by its `schema_ref` name.
///
/// The `revy plan schema --packet <kind>` surface, derived from
/// [`WorkKind::ALL`] so a new kind is resolvable without editing this function.
pub fn packet_schema_by_name(name: &str) -> Option<serde_json::Value> {
    WorkKind::ALL
        .into_iter()
        .find(|kind| kind.name() == name)
        .map(schema_for)
}

/// Every packet kind's `schema_ref` name, in canonical order — for a
/// help/enumeration surface. Derived from [`WorkKind::ALL`].
pub fn packet_kind_names() -> Vec<&'static str> {
    WorkKind::ALL.iter().map(|kind| kind.name()).collect()
}

/// The JSON Schema of the [`WorkKind`] tag enum.
///
/// Its `oneOf` has one entry per variant, so the release-invariant test counts it
/// to couple "exactly ten kinds" to the enum rather than to a hand-maintained
/// list (design §18.5).
pub fn work_kind_schema() -> serde_json::Value {
    result_schema::<WorkKind>()
}

/// One unit of ready work: its stable key, kind, subject, one-sentence mandate,
/// kind-specific context the worker needs, and the `schema_ref` naming its result
/// schema (design §7).
#[derive(Debug, Clone, Serialize)]
pub struct ReadyPacket {
    key: WorkKey,
    #[serde(flatten)]
    kind: WorkKind,
    subject: WorkSubject,
    mandate: NonEmptyText,
    context: serde_json::Value,
    schema_ref: &'static str,
}

impl ReadyPacket {
    pub(crate) fn new(
        kind: WorkKind,
        subject: WorkSubject,
        mandate: NonEmptyText,
        context: serde_json::Value,
    ) -> Self {
        let key = WorkKey::derive(kind, &subject);
        Self {
            key,
            kind,
            subject,
            mandate,
            context,
            schema_ref: kind.name(),
        }
    }

    pub const fn key(&self) -> &WorkKey {
        &self.key
    }

    pub const fn kind(&self) -> WorkKind {
        self.kind
    }

    pub const fn subject(&self) -> &WorkSubject {
        &self.subject
    }

    pub const fn mandate(&self) -> &NonEmptyText {
        &self.mandate
    }

    pub const fn context(&self) -> &serde_json::Value {
        &self.context
    }

    /// The JSON Schema the result to this packet must satisfy — inlined by
    /// `status --schema`.
    pub fn result_schema(&self) -> serde_json::Value {
        schema_for(self.kind)
    }

    /// Decode packet bytes into the raw result this packet's kind consumes.
    pub fn decode_result(&self, bytes: &[u8]) -> Result<RawWorkResult, DecodeError> {
        decode(self.kind, bytes)
    }
}

/// A frontier could not be narrowed to a single packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("frontier holds {count} packets, not exactly one")]
pub struct FrontierCardinalityError {
    pub count: usize,
}

/// The non-empty set of ready packets for the current run state.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct ReadyFrontier(OneOrMore<ReadyPacket>);

impl ReadyFrontier {
    pub(crate) fn single(packet: ReadyPacket) -> Self {
        Self(OneOrMore::from_one(packet))
    }

    pub(crate) const fn many(packets: OneOrMore<ReadyPacket>) -> Self {
        Self(packets)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ReadyPacket> {
        self.0.iter()
    }

    pub fn kinds(&self) -> Vec<WorkKind> {
        self.0.iter().map(ReadyPacket::kind).collect()
    }

    /// The sole ready packet, or a cardinality error when the frontier is not a
    /// singleton.
    pub fn only(&self) -> Result<&ReadyPacket, FrontierCardinalityError> {
        match self.0.as_slice() {
            [packet] => Ok(packet),
            other => Err(FrontierCardinalityError { count: other.len() }),
        }
    }
}

impl<'a> IntoIterator for &'a ReadyFrontier {
    type Item = &'a ReadyPacket;
    type IntoIter = std::slice::Iter<'a, ReadyPacket>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// The work a run offers next: a non-empty ready frontier, or a terminal (the
/// final plan owes no further work).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Frontier {
    Ready(ReadyFrontier),
    Terminal,
}

impl Frontier {
    pub fn iter(&self) -> std::slice::Iter<'_, ReadyPacket> {
        match self {
            Self::Ready(ready) => ready.iter(),
            Self::Terminal => [].iter(),
        }
    }

    pub fn kinds(&self) -> Vec<WorkKind> {
        match self {
            Self::Ready(ready) => ready.kinds(),
            Self::Terminal => Vec::new(),
        }
    }

    /// The sole ready packet, or a cardinality error. A terminal frontier reports
    /// a count of zero.
    pub fn only(&self) -> Result<&ReadyPacket, FrontierCardinalityError> {
        match self {
            Self::Ready(ready) => ready.only(),
            Self::Terminal => Err(FrontierCardinalityError { count: 0 }),
        }
    }
}

impl<'a> IntoIterator for &'a Frontier {
    type Item = &'a ReadyPacket;
    type IntoIter = std::slice::Iter<'a, ReadyPacket>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
