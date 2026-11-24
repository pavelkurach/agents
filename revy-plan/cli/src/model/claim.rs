use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{AcId, ClaimId, DecisionId, EvidenceId, ProposalId, UncertaintyId};
use crate::primitives::{NonEmptyText, OneOrMore};

/// How far a claim's evidence carries it: fully established, or established with a stated limit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClaimConclusion {
    Established,
    Limited { limitation: NonEmptyText },
}

/// A statement about the codebase that stands on named evidence and states its consequences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: ClaimId,
    pub statement: NonEmptyText,
    pub evidence: OneOrMore<EvidenceId>,
    pub consequences: OneOrMore<NonEmptyText>,
    pub conclusion: ClaimConclusion,
}

/// What was done with an uncertainty: still open, contained by an acceptance
/// criterion, or accepted by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum UncertaintyDisposition {
    Open {
        proposed_capability: Option<ProposalId>,
    },
    ContainedByCriterion {
        criterion: AcId,
    },
    AcceptedByUser {
        decision: DecisionId,
    },
}

/// A named uncertainty surfaced during exploration together with how it was disposed of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedFact {
    pub id: UncertaintyId,
    pub question: NonEmptyText,
    pub disposition: UncertaintyDisposition,
}
