use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::evidence::EvidenceClass;
use crate::ids::{CapabilityId, ProposalId, UncertaintyId};
use crate::primitives::{NonEmptyText, OneOrMore, RepoRelPath, Sha256Digest};

/// A repository state (revision plus dirty-tree digest) that evidence is bound to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryIdentity {
    pub revision: NonEmptyText,
    pub dirty_digest: Sha256Digest,
}

/// A side effect that installing or invoking a proposed tool would impose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SetupEffect {
    Network,
    Download,
    PersistentConfig { path: NonEmptyText },
    TargetWrite { path: RepoRelPath },
    DataEgress { destination: NonEmptyText },
}

/// A capability the environment already offers, with the evidence classes it can
/// prove.
///
/// Setup runs no probe (design §6.7): an approved tool proves itself by appearing
/// as evidence provenance in the next exploration, not by exiting 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AvailableCapability {
    pub id: CapabilityId,
    pub name: NonEmptyText,
    pub version: Option<NonEmptyText>,
    pub interface: NonEmptyText,
    pub proves: OneOrMore<EvidenceClass>,
    pub limitations: Vec<NonEmptyText>,
}

/// The reviewer's standing on a proposed tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    AddNow,
    Optional,
    Unnecessary,
}

/// A request to add a tool, disclosing its cost and side effects up front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolProposal {
    pub id: ProposalId,
    pub name: NonEmptyText,
    pub closes: OneOrMore<UncertaintyId>,
    pub benefit: NonEmptyText,
    pub cost: NonEmptyText,
    pub side_effects: Vec<SetupEffect>,
    pub recommendation: Recommendation,
}

/// The full picture presented for a capability decision: what exists and what is proposed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityReport {
    pub repository: RepositoryIdentity,
    pub capabilities: Vec<AvailableCapability>,
    pub proposals: Vec<ToolProposal>,
}

impl CapabilityReport {
    /// Whether the report leaves the user nothing to decide: it proposes no
    /// addition, and code intelligence (a capability proving `semantic`
    /// evidence) is already available.
    pub fn needs_no_approval(&self) -> bool {
        self.proposals.is_empty()
            && self.capabilities.iter().any(|capability| {
                capability
                    .proves
                    .iter()
                    .any(|class| *class == EvidenceClass::Semantic)
            })
    }
}

/// A capability report as an author supplies it.
///
/// Accepted by `init`, a `capability_report` packet, and an exploration's
/// `propose_capability`. The repository identity may be omitted, because the
/// CLI stamps the plan's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityReportInput {
    /// Optional; replaced by the plan's repository identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositoryIdentity>,
    pub capabilities: Vec<AvailableCapability>,
    pub proposals: Vec<ToolProposal>,
}

impl CapabilityReportInput {
    /// The report bound to the captured repository identity, which replaces any
    /// identity the author supplied.
    #[must_use]
    pub fn stamped(self, repository: RepositoryIdentity) -> CapabilityReport {
        CapabilityReport {
            repository,
            capabilities: self.capabilities,
            proposals: self.proposals,
        }
    }
}

/// A human's decision over a [`CapabilityReport`], bound to that report's digest.
///
/// The digest may be omitted: the decision then binds to the report of the
/// packet it answers, which the work key already pins to the current report.
/// A digest given is checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapabilityDecision {
    ProceedCurrent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = DIGEST_DESCRIPTION)]
        report: Option<Sha256Digest>,
    },
    ApproveAdditions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = DIGEST_DESCRIPTION)]
        report: Option<Sha256Digest>,
        proposals: OneOrMore<ProposalId>,
    },
    RequestRevision {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(description = DIGEST_DESCRIPTION)]
        report: Option<Sha256Digest>,
        instruction: NonEmptyText,
    },
}

const DIGEST_DESCRIPTION: &str = "Optional: the capability report's SHA-256 digest (64 lowercase hex). Omitted, the decision binds to the report of the packet it answers; given, it must match.";

impl CapabilityDecision {
    /// The digest the decision declares, if any.
    pub const fn declared_report(&self) -> Option<&Sha256Digest> {
        match self {
            Self::ProceedCurrent { report }
            | Self::ApproveAdditions { report, .. }
            | Self::RequestRevision { report, .. } => report.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[test]
    fn proposed_tool_requires_cost_and_side_effects() {
        let raw = r#"{
          "id":"proposal-rust-analyzer",
          "name":"rust-analyzer",
          "closes":["uncertainty-consumers"],
          "benefit":"Resolve Rust call edges.",
          "recommendation":"add_now"
        }"#;

        serde_json::from_str::<ToolProposal>(raw)
            .expect_err("undisclosed setup is not an approvable proposal");
    }

    #[test]
    fn approvable_proposal_round_trips() {
        let raw = r#"{
          "id":"proposal-rust-analyzer",
          "name":"rust-analyzer",
          "closes":["uncertainty-consumers"],
          "benefit":"Resolve Rust call edges.",
          "cost":"Downloads a language server binary.",
          "side_effects":[{"kind":"download"},{"kind":"network"}],
          "recommendation":"add_now"
        }"#;

        let proposal: ToolProposal = serde_json::from_str(raw).expect("valid proposal");
        assert_eq!(proposal.recommendation, Recommendation::AddNow);
        assert_eq!(proposal.side_effects.len(), 2);
    }

    #[test]
    fn available_capability_requires_at_least_one_proven_class() {
        let raw = r#"{
          "id":"capability-lsp",
          "name":"rust-analyzer",
          "version":null,
          "interface":"lsp",
          "proves":[],
          "limitations":[]
        }"#;

        serde_json::from_str::<AvailableCapability>(raw)
            .expect_err("a capability that proves nothing is not admissible");
    }

    #[test]
    fn available_capability_round_trips() {
        let raw = r#"{
          "id":"capability-lsp",
          "name":"rust-analyzer",
          "version":null,
          "interface":"lsp",
          "proves":["semantic"],
          "limitations":[]
        }"#;

        let capability: AvailableCapability = serde_json::from_str(raw).expect("valid capability");
        assert_eq!(capability.proves.as_slice().len(), 1);
        assert_eq!(capability.interface.as_str(), "lsp");
    }

    #[test]
    fn capability_decision_rejects_an_unknown_decision() {
        let raw = format!(r#"{{"decision":"defer","report":"{DIGEST}"}}"#);
        serde_json::from_str::<CapabilityDecision>(&raw)
            .expect_err("only the closed decision set is admissible");
    }

    #[test]
    fn approve_additions_decision_round_trips() {
        let raw = format!(
            r#"{{"decision":"approve_additions","report":"{DIGEST}","proposals":["proposal-rust-analyzer"]}}"#
        );
        let decision: CapabilityDecision = serde_json::from_str(&raw).expect("valid decision");
        match decision {
            CapabilityDecision::ApproveAdditions { proposals, .. } => {
                assert_eq!(proposals.as_slice().len(), 1);
            }
            other => panic!("expected approve_additions, got {other:?}"),
        }
    }
}
