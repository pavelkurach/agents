use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::RepositoryIdentity;
use crate::command::CommandSpec;
use crate::ids::EvidenceId;
use crate::primitives::{Location, NonEmptyText, OneOrMore, RepoAlias, RepoRelPath, RepoScope};

/// The kind of evidence a capability can produce.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    Runtime,
    Semantic,
    Structural,
    Textual,
    Documentation,
    History,
}

/// Whether a result set is exhaustive, or a sample of a larger population.
///
/// A `Sampled { total }` set records how many items exist so the renderer can
/// mark evidence that was pruned; `total` must be at least the number of retained
/// items, or the sample is dishonest (design §6.1). No CLI re-execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Completeness {
    Complete,
    Sampled { total: u32 },
}

impl Completeness {
    /// Whether a sample of `total` is honest about `retained` items actually
    /// carried: a complete set always is; a sample must claim at least as many
    /// total as it retained.
    pub const fn admits(self, retained: usize) -> bool {
        match self {
            Self::Complete => true,
            Self::Sampled { total } => total as usize >= retained,
        }
    }

    /// Whether this result set was sampled (pruned), for the renderer's marker.
    pub const fn is_sampled(self) -> bool {
        matches!(self, Self::Sampled { .. })
    }
}

/// Which tool produced a semantic or structural observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ToolProvenance {
    pub tool: NonEmptyText,
    pub version: Option<NonEmptyText>,
}

/// A reference to a captured transcript artifact plus its content digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TranscriptRef {
    pub path: RepoRelPath,
    pub digest: crate::primitives::Sha256Digest,
}

/// A semantic reference occurrence at a location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub location: Location,
    pub context: Option<NonEmptyText>,
}

/// A directed edge in a resolved call graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CallEdge {
    pub caller: Location,
    pub callee: Location,
}

/// A single textual search match: where it occurred and the matched line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextMatch {
    pub location: Location,
    pub line: NonEmptyText,
}

/// What a semantic (name-resolved) observation established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticObservation {
    Definitions {
        locations: OneOrMore<Location>,
    },
    References {
        references: Vec<Reference>,
        completeness: Completeness,
    },
    Implementations {
        locations: Vec<Location>,
        completeness: Completeness,
    },
    CallEdges {
        edges: Vec<CallEdge>,
        completeness: Completeness,
    },
}

impl SemanticObservation {
    /// Whether this observation's declared completeness is honest about the
    /// number of items it retained. `Definitions` carries no completeness and is
    /// always admissible.
    const fn completeness_admits(&self) -> bool {
        match self {
            Self::Definitions { .. } => true,
            Self::References {
                references,
                completeness,
            } => completeness.admits(references.len()),
            Self::Implementations {
                locations,
                completeness,
            } => completeness.admits(locations.len()),
            Self::CallEdges {
                edges,
                completeness,
            } => completeness.admits(edges.len()),
        }
    }
}

/// What a structural (syntax/symbol-level) observation established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum StructuralObservation {
    Symbols {
        locations: OneOrMore<Location>,
        completeness: Completeness,
    },
    Imports {
        modules: OneOrMore<NonEmptyText>,
    },
}

impl StructuralObservation {
    /// Whether this observation's declared completeness is honest about the
    /// number of items it retained. `Imports` carries no completeness.
    fn completeness_admits(&self) -> bool {
        match self {
            Self::Symbols {
                locations,
                completeness,
            } => completeness.admits(locations.as_slice().len()),
            Self::Imports { .. } => true,
        }
    }
}

/// Evidence gathered by executing a command against a known repository state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeEvidence {
    pub id: EvidenceId,
    pub command: CommandSpec,
    pub target: RepositoryIdentity,
    pub observation: NonEmptyText,
    pub artifact: TranscriptRef,
    pub limitation: Option<NonEmptyText>,
}

/// Evidence gathered from name-resolving (LSP / code-graph) tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SemanticEvidence {
    pub id: EvidenceId,
    pub provenance: ToolProvenance,
    pub operation: NonEmptyText,
    pub subject: NonEmptyText,
    pub observation: SemanticObservation,
    /// The repository state this was observed against. Optional while a plan is
    /// in flight; a verified plan is proven to ground every name-resolved
    /// observation on its own repository identity (see `document.rs`), so a read
    /// taken against a stale checkout cannot masquerade as current.
    #[serde(default)]
    pub target: Option<RepositoryIdentity>,
    pub limitation: Option<NonEmptyText>,
}

/// Evidence gathered from syntax/symbol-level inspection without full resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StructuralEvidence {
    pub id: EvidenceId,
    pub provenance: ToolProvenance,
    pub subject: NonEmptyText,
    pub observation: StructuralObservation,
    /// The repository state this was observed against — see [`SemanticEvidence::target`].
    #[serde(default)]
    pub target: Option<RepositoryIdentity>,
    pub limitation: Option<NonEmptyText>,
}

/// Evidence gathered from textual search over a bounded scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextualEvidence {
    pub id: EvidenceId,
    pub scope: OneOrMore<RepoScope>,
    pub pattern: NonEmptyText,
    pub matches: Vec<TextMatch>,
    /// Whether the recorded matches are every match, or a sample of a larger set.
    pub completeness: Completeness,
    /// The repository state this was observed against — see [`SemanticEvidence::target`].
    #[serde(default)]
    pub target: Option<RepositoryIdentity>,
    pub limitation: Option<NonEmptyText>,
}

/// Evidence drawn from a documented statement in a specific section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentationEvidence {
    pub id: EvidenceId,
    pub document: NonEmptyText,
    pub section: NonEmptyText,
    pub observation: NonEmptyText,
    /// The repository state this was observed against — see [`SemanticEvidence::target`].
    #[serde(default)]
    pub target: Option<RepositoryIdentity>,
    pub limitation: Option<NonEmptyText>,
}

/// Evidence drawn from repository history over a revision range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryEvidence {
    pub id: EvidenceId,
    pub repository: RepositoryIdentity,
    pub range: NonEmptyText,
    pub operation: NonEmptyText,
    pub observation: NonEmptyText,
    pub limitation: Option<NonEmptyText>,
}

/// A conclusion inferred from other evidence, with the fact that would refute it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InferenceEvidence {
    pub id: EvidenceId,
    pub premises: OneOrMore<EvidenceId>,
    pub conclusion: NonEmptyText,
    pub falsifier: NonEmptyText,
}

/// A single piece of evidence, discriminated by how it was obtained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Evidence {
    Runtime(RuntimeEvidence),
    Semantic(SemanticEvidence),
    Structural(StructuralEvidence),
    Textual(TextualEvidence),
    Documentation(DocumentationEvidence),
    History(HistoryEvidence),
    Inference(InferenceEvidence),
}

impl Evidence {
    /// The repository state this evidence was observed against, if it carries
    /// one. `Runtime`/`History` always carry an identity; the name-resolved,
    /// structural, textual, and documentation kinds carry an optional one that a
    /// verified plan must fill; `Inference` is derived from other evidence and
    /// carries none of its own.
    pub const fn observed_target(&self) -> Option<&RepositoryIdentity> {
        match self {
            Self::Runtime(evidence) => Some(&evidence.target),
            Self::History(evidence) => Some(&evidence.repository),
            Self::Semantic(evidence) => evidence.target.as_ref(),
            Self::Structural(evidence) => evidence.target.as_ref(),
            Self::Textual(evidence) => evidence.target.as_ref(),
            Self::Documentation(evidence) => evidence.target.as_ref(),
            Self::Inference(_) => None,
        }
    }

    /// The optional `target` slot of the kinds that may omit it on input (and
    /// that the CLI stamps); `None` for kinds whose identity is required or
    /// absent.
    pub(crate) const fn optional_target_mut(&mut self) -> Option<&mut Option<RepositoryIdentity>> {
        match self {
            Self::Semantic(evidence) => Some(&mut evidence.target),
            Self::Structural(evidence) => Some(&mut evidence.target),
            Self::Textual(evidence) => Some(&mut evidence.target),
            Self::Documentation(evidence) => Some(&mut evidence.target),
            Self::Runtime(_) | Self::History(_) | Self::Inference(_) => None,
        }
    }

    /// Every source location this evidence anchors to.
    pub fn locations(&self) -> Vec<&Location> {
        match self {
            Self::Textual(evidence) => evidence
                .matches
                .iter()
                .map(|occurrence| &occurrence.location)
                .collect(),
            Self::Semantic(evidence) => match &evidence.observation {
                SemanticObservation::Definitions { locations } => locations.iter().collect(),
                SemanticObservation::References { references, .. } => references
                    .iter()
                    .map(|reference| &reference.location)
                    .collect(),
                SemanticObservation::Implementations { locations, .. } => {
                    locations.iter().collect()
                }
                SemanticObservation::CallEdges { edges, .. } => edges
                    .iter()
                    .flat_map(|edge| [&edge.caller, &edge.callee])
                    .collect(),
            },
            Self::Structural(evidence) => match &evidence.observation {
                StructuralObservation::Symbols { locations, .. } => locations.iter().collect(),
                StructuralObservation::Imports { .. } => Vec::new(),
            },
            Self::Runtime(_) | Self::Documentation(_) | Self::History(_) | Self::Inference(_) => {
                Vec::new()
            }
        }
    }

    /// The distinct repositories this evidence's anchors and scopes cite, in
    /// order; `None` is the primary repository. Empty for anchorless evidence.
    pub fn cited_repos(&self) -> BTreeSet<Option<&RepoAlias>> {
        let scopes = match self {
            Self::Textual(evidence) => evidence.scope.iter().map(RepoScope::repo).collect(),
            _ => Vec::new(),
        };
        self.locations()
            .into_iter()
            .map(|location| location.path.repo())
            .chain(scopes)
            .collect()
    }

    /// Whether a verified plan must ground this evidence on its repository
    /// identity. Every kind except `Inference` — which is a conclusion drawn from
    /// grounded premises, not a fresh observation — must carry a matching target.
    pub const fn requires_grounding(&self) -> bool {
        !matches!(self, Self::Inference(_))
    }

    /// Whether every completeness this evidence declares is honest about the
    /// number of items it retained (design §6.1). A `Sampled { total }` set whose
    /// `total` is below the retained count is a dishonest sample and is rejected
    /// at the exploration and document-load boundaries.
    pub fn sample_admits(&self) -> bool {
        match self {
            Self::Semantic(evidence) => evidence.observation.completeness_admits(),
            Self::Structural(evidence) => evidence.observation.completeness_admits(),
            Self::Textual(evidence) => evidence.completeness.admits(evidence.matches.len()),
            Self::Runtime(_) | Self::Documentation(_) | Self::History(_) | Self::Inference(_) => {
                true
            }
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
    fn evidence_rejects_an_unknown_kind() {
        serde_json::from_str::<Evidence>(r#"{"kind":"telepathy","id":"evidence-x"}"#)
            .expect_err("unknown evidence kinds are rejected");
    }

    #[test]
    fn runtime_evidence_requires_an_observation() {
        let raw = format!(
            r#"{{
              "kind":"runtime",
              "id":"evidence-build",
              "command":{{"kind":"program","program":"cargo","args":["build"]}},
              "target":{{"revision":"abc123","dirty_digest":"{DIGEST}"}},
              "artifact":{{"path":"transcripts/build.txt","digest":"{DIGEST}"}},
              "limitation":null
            }}"#
        );
        serde_json::from_str::<Evidence>(&raw)
            .expect_err("runtime evidence must record an observation");
    }

    #[test]
    fn runtime_evidence_round_trips() {
        let raw = format!(
            r#"{{
              "kind":"runtime",
              "id":"evidence-build",
              "command":{{"kind":"program","program":"cargo","args":["build"]}},
              "target":{{"revision":"abc123","dirty_digest":"{DIGEST}"}},
              "observation":"The workspace builds cleanly.",
              "artifact":{{"path":"transcripts/build.txt","digest":"{DIGEST}"}},
              "limitation":null
            }}"#
        );
        let evidence: Evidence = serde_json::from_str(&raw).expect("valid runtime evidence");
        assert!(matches!(evidence, Evidence::Runtime(_)));
    }

    #[test]
    fn semantic_definitions_evidence_round_trips() {
        let raw = r#"{
          "kind":"semantic",
          "id":"evidence-defs",
          "provenance":{"tool":"serena","version":null},
          "operation":"find_symbol",
          "subject":"CapabilityReport",
          "observation":{"shape":"definitions","locations":[{"path":"src/capability.rs","line":10,"column":1}]},
          "limitation":null
        }"#;
        let evidence: Evidence = serde_json::from_str(raw).expect("valid semantic evidence");
        match evidence {
            Evidence::Semantic(semantic) => match semantic.observation {
                SemanticObservation::Definitions { locations } => {
                    assert_eq!(locations.as_slice().len(), 1);
                }
                other => panic!("expected definitions, got {other:?}"),
            },
            other => panic!("expected semantic evidence, got {other:?}"),
        }
    }

    #[test]
    fn semantic_definitions_require_at_least_one_location() {
        let raw = r#"{
          "kind":"semantic",
          "id":"evidence-defs",
          "provenance":{"tool":"serena","version":null},
          "operation":"find_symbol",
          "subject":"CapabilityReport",
          "observation":{"shape":"definitions","locations":[]},
          "limitation":null
        }"#;
        serde_json::from_str::<Evidence>(raw)
            .expect_err("a definitions observation must carry at least one location");
    }

    #[test]
    fn inference_evidence_requires_premises() {
        let raw = r#"{
          "kind":"inference",
          "id":"evidence-inferred",
          "premises":[],
          "conclusion":"The port is unused.",
          "falsifier":"A caller referencing the port."
        }"#;
        serde_json::from_str::<Evidence>(raw)
            .expect_err("an inference must stand on at least one premise");
    }

    #[test]
    fn inference_evidence_needs_no_grounding_but_observations_do() {
        // Inference is derived from grounded premises, so it carries no target
        // and needs none; a fresh semantic observation requires grounding.
        let inference: Evidence = serde_json::from_str(
            r#"{"kind":"inference","id":"e1","premises":["e0"],"conclusion":"unused.","falsifier":"a caller."}"#,
        )
        .expect("inference parses");
        assert!(!inference.requires_grounding());
        assert!(inference.observed_target().is_none());

        let semantic: Evidence = serde_json::from_str(
            r#"{"kind":"semantic","id":"e2","provenance":{"tool":"serena","version":null},"operation":"find_symbol","subject":"x","observation":{"shape":"references","references":[],"completeness":{"kind":"complete"}},"limitation":null}"#,
        )
        .expect("semantic parses without a target");
        assert!(semantic.requires_grounding());
        // Optional in flight: absent until a verified plan requires it.
        assert!(semantic.observed_target().is_none());
    }

    #[test]
    fn references_observation_requires_completeness() {
        let raw = r#"{
          "kind":"semantic",
          "id":"evidence-refs",
          "provenance":{"tool":"serena","version":null},
          "operation":"find_referencing_symbols",
          "subject":"parse",
          "observation":{"shape":"references","references":[]},
          "limitation":null
        }"#;
        serde_json::from_str::<Evidence>(raw)
            .expect_err("a references observation must declare completeness");
    }

    #[test]
    fn a_sample_claiming_fewer_than_it_retains_is_rejected() {
        let raw = r#"{
              "kind":"textual",
              "id":"evidence-grep",
              "scope":["src/lib.rs"],
              "pattern":"parse",
              "matches":[
                {"location":{"path":"src/lib.rs","line":1,"column":1},"line":"parse()"},
                {"location":{"path":"src/lib.rs","line":2,"column":1},"line":"parse()"}
              ],
              "completeness":{"kind":"sampled","total":1},
              "target":null,
              "limitation":null
            }"#
        .to_string();
        let evidence: Evidence = serde_json::from_str(&raw).expect("shape parses");
        assert!(
            !evidence.sample_admits(),
            "a sample of total 1 that retained 2 matches is dishonest"
        );
    }
}
