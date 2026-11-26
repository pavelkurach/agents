use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{DecisionId, RiskId};
use crate::model::diagram::Diagram;
use crate::primitives::{NonEmptyText, OneOrMore, RepoPath};

/// How a symbol the delta names changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SymbolChange {
    Added,
    Modified,
    Removed,
}

/// One symbol the change touches, the file it lives in, and how it changes.
///
/// Every path must appear in some `Mr.files` and every symbol is unique across
/// the candidate (proven at candidate parse, design §6.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChangedSymbol {
    pub symbol: NonEmptyText,
    pub path: RepoPath,
    pub change: SymbolChange,
}

/// A review focus that a specialist lens covers, on top of the integrated
/// review. Closed: a plan can only surface a risk against one of these lenses.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistFocus {
    Concurrency,
    Migration,
    Security,
    ProtocolCompatibility,
    Performance,
    PublicApi,
}

/// A risk the plan discloses, discriminated by whether it needs a specialist.
///
/// A `Specialist` risk names the focus whose review it demands; an `Ordinary`
/// risk is covered by the integrated review alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Risk {
    Ordinary {
        id: RiskId,
        statement: NonEmptyText,
        mitigation: NonEmptyText,
    },
    Specialist {
        id: RiskId,
        focus: SpecialistFocus,
        statement: NonEmptyText,
        mitigation: NonEmptyText,
    },
}

impl Risk {
    /// The stable id this risk carries, regardless of kind.
    pub const fn id(&self) -> &RiskId {
        match self {
            Self::Ordinary { id, .. } | Self::Specialist { id, .. } => id,
        }
    }

    /// The specialist focus this risk demands, or `None` when the integrated
    /// review covers it. Passed to `Iterator::filter_map` to collect the
    /// distinct specialist set a draft requires.
    pub const fn specialist_focus(&self) -> Option<SpecialistFocus> {
        match self {
            Self::Specialist { focus, .. } => Some(*focus),
            Self::Ordinary { .. } => None,
        }
    }
}

/// Who owns a design decision: the planner made it, or the user must.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOwner {
    Planner,
    User,
}

/// An alternative a settled decision considered and rejected, and why it lost.
///
/// The negation that makes a design legible ("there is no `CancelBlockedPop` —
/// the actor already serializes every pop") gets a first-class place here rather
/// than being smuggled into prose or dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuledOut {
    pub alternative: NonEmptyText,
    pub reason: NonEmptyText,
}

/// Whether a decision is still open, or answered with its consequences recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecisionState {
    Proposed,
    Answered {
        answer: NonEmptyText,
        consequences: OneOrMore<NonEmptyText>,
        // A settled decision may record the alternatives it rejected. Absent
        // means none were disclosed, so every existing plan stays valid.
        #[serde(default)]
        ruled_out: Vec<RuledOut>,
    },
}

/// A single design decision, owned and tracked through its resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: DecisionId,
    pub owner: DecisionOwner,
    pub question: NonEmptyText,
    pub state: DecisionState,
    /// The note the user gave with their answer to a user-owned decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<NonEmptyText>,
}

/// A design decision proposed while authoring the brief, discriminated by owner.
///
/// A `User`-owned decision must disclose the alternatives the user chooses
/// between; a `Planner`-owned decision instead records the answer the planner
/// already settled. Requiring alternatives only for user-owned decisions is
/// structural — the owner tag selects which fields are present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "owner", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedDecision {
    Planner {
        id: DecisionId,
        question: NonEmptyText,
        answer: NonEmptyText,
        consequences: OneOrMore<NonEmptyText>,
        // The alternatives the planner rejected in settling this decision.
        // `User`-owned decisions omit this: their alternatives are still live
        // for the user to choose between, not yet ruled out.
        #[serde(default)]
        ruled_out: Vec<RuledOut>,
    },
    User {
        id: DecisionId,
        question: NonEmptyText,
        alternatives: OneOrMore<NonEmptyText>,
        consequences: OneOrMore<NonEmptyText>,
    },
}

impl ProposedDecision {
    pub const fn id(&self) -> &DecisionId {
        match self {
            Self::Planner { id, .. } | Self::User { id, .. } => id,
        }
    }
}

/// One open question put to the user: the decision it settles, the question
/// text, and the closed set of options they choose between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserQuestion {
    pub decision: DecisionId,
    pub question: NonEmptyText,
    pub options: OneOrMore<NonEmptyText>,
}

/// The user's answer to one open question: the decision it settles, the option
/// they chose (which must be one of that question's options), and an optional
/// note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserAnswer {
    pub decision: DecisionId,
    pub chosen: NonEmptyText,
    #[serde(default)]
    pub note: Option<NonEmptyText>,
}

/// The approved problem framing that scopes a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignBrief {
    /// A short headline for the rendered plan's `<h1>` — a few words, not a
    /// sentence. Absent on briefs written before titles existed; the renderer
    /// then falls back to a truncated `summary`.
    #[serde(default)]
    pub title: Option<NonEmptyText>,
    pub summary: NonEmptyText,
    pub scope: NonEmptyText,
    pub out_of_scope: NonEmptyText,
}

/// A source language a code excerpt is highlighted as. Closed: the planner may
/// only author an excerpt in a language the renderer can highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    // `rename_all = "snake_case"` would otherwise produce "type_script" /
    // "java_script" — these compound names are single words on the wire.
    #[serde(rename = "typescript")]
    TypeScript,
    #[serde(rename = "javascript")]
    JavaScript,
    Go,
    Json,
    Yaml,
    Toml,
    Bash,
    Sql,
    Text,
}

/// A syntax-highlighted excerpt of source code, authored as part of the plan
/// narrative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CodeExcerpt {
    pub lang: Language,
    pub caption: Option<NonEmptyText>,
    pub code: NonEmptyText,
}

/// One glossary entry: a term paired with its definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GlossaryTerm {
    pub term: NonEmptyText,
    pub definition: NonEmptyText,
}

/// Reader orientation for the plan narrative: where the change sits, which
/// flows it touches, and any terms the reader may need defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Orientation {
    pub placement: NonEmptyText,
    #[serde(default)]
    pub touched_flows: Vec<NonEmptyText>,
    #[serde(default)]
    pub glossary: Vec<GlossaryTerm>,
}

/// One block of authored plan narrative. Closed sum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Block {
    Prose { text: NonEmptyText },
    Code { excerpt: CodeExcerpt },
    Figure { diagram: Diagram },
}

/// One heading and the non-empty sequence of narrative blocks under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub heading: NonEmptyText,
    pub blocks: OneOrMore<Block>,
}

/// The human-facing plan narrative. Design decisions are carried by the CLI on
/// the candidate (design §6.4), not authored here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanContent {
    pub current_behavior: NonEmptyText,
    pub target_behavior: NonEmptyText,
    pub changed_symbols: OneOrMore<ChangedSymbol>,
    // A draft with no disclosed risk is admissible (a purely local change), so
    // an absent field means "no risks" rather than an incomplete document.
    #[serde(default)]
    pub risks: Vec<Risk>,
    // Authored narrative is optional: a draft with no orientation or sections
    // is still a coherent, if minimal, plan.
    #[serde(default)]
    pub orientation: Option<Orientation>,
    #[serde(default)]
    pub sections: Vec<Section>,
}

#[cfg(test)]
mod authored_narrative_tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use super::*;

    #[test]
    fn test_section_rejects_an_empty_blocks_array() {
        let json = r#"{"heading":"Overview","blocks":[]}"#;

        serde_json::from_str::<Section>(json).expect_err("empty blocks must be rejected");
    }

    #[test]
    fn test_planner_decision_parses_without_ruled_out() {
        // Backward compat: a brief packet authored before `ruled_out` existed
        // omits it; `#[serde(default)]` must still parse the Planner decision.
        let json = r#"{"owner":"planner","id":"d1","question":"Which container backs the queue?","answer":"VecDeque behind a mutex","consequences":["No new dependency."]}"#;

        let decision: ProposedDecision =
            serde_json::from_str(json).expect("a planner decision parses without ruled_out");

        match decision {
            ProposedDecision::Planner { ruled_out, .. } => assert_eq!(ruled_out.as_slice(), []),
            ProposedDecision::User { .. } => panic!("expected a planner decision, got a user one"),
        }
    }

    #[test]
    fn test_answered_decision_state_parses_without_ruled_out() {
        // Same backward-compat guarantee for the final plan's answered state.
        let json = r#"{"state":"answered","answer":"Bound at 1024","consequences":["Producer blocks when full."]}"#;

        let state: DecisionState =
            serde_json::from_str(json).expect("an answered state parses without ruled_out");

        match state {
            DecisionState::Answered { ruled_out, .. } => assert_eq!(ruled_out.as_slice(), []),
            DecisionState::Proposed => panic!("expected an answered state, got a proposed one"),
        }
    }

    #[test]
    fn test_code_excerpt_rejects_empty_code() {
        let json = r#"{"lang":"rust","caption":null,"code":""}"#;

        serde_json::from_str::<CodeExcerpt>(json).expect_err("empty code must be rejected");
    }

    #[test]
    fn test_code_excerpt_rejects_an_unknown_field() {
        let json = r#"{"lang":"rust","caption":null,"code":"fn main() {}","extra":true}"#;

        serde_json::from_str::<CodeExcerpt>(json).expect_err("unknown fields must be rejected");
    }

    #[test]
    fn test_language_serializes_compound_names_as_single_words() {
        // Regression test: a bare `rename_all = "snake_case"` would produce
        // "type_script" / "java_script", which no author would think to
        // write and `revy plan schema` would then advertise as correct.
        assert_eq!(
            serde_json::to_value(Language::TypeScript).expect("language serializes"),
            serde_json::json!("typescript")
        );
        assert_eq!(
            serde_json::to_value(Language::JavaScript).expect("language serializes"),
            serde_json::json!("javascript")
        );
        assert_eq!(
            serde_json::from_value::<Language>(serde_json::json!("typescript"))
                .expect("\"typescript\" parses"),
            Language::TypeScript
        );
        assert_eq!(
            serde_json::from_value::<Language>(serde_json::json!("javascript"))
                .expect("\"javascript\" parses"),
            Language::JavaScript
        );
    }

    #[test]
    fn test_plan_content_with_orientation_and_a_code_section_round_trips() {
        let json = serde_json::json!({
            "current_behavior": "Work runs inline.",
            "target_behavior": "Work runs on a queue.",
            "changed_symbols": [
                {"symbol": "submit_job", "path": "src/api.rs", "change": "modified"}
            ],
            "risks": [],
            "orientation": {
                "placement": "Sits between the API handler and the worker pool.",
                "touched_flows": ["submit-job"],
                "glossary": [
                    {"term": "worker", "definition": "A process that drains the queue."}
                ]
            },
            "sections": [
                {
                    "heading": "Queue interface",
                    "blocks": [
                        {"kind": "prose", "text": "The queue exposes a bounded push/pop API."},
                        {
                            "kind": "code",
                            "excerpt": {
                                "lang": "rust",
                                "caption": "push",
                                "code": "fn push(&self, item: Job) -> Result<(), Full> { .. }"
                            }
                        }
                    ]
                }
            ]
        });
        let raw = serde_json::to_vec(&json).expect("json serializes");

        let content: PlanContent = serde_json::from_slice(&raw).expect("plan content parses");
        let reserialized = serde_json::to_value(&content).expect("plan content serializes");

        assert_eq!(reserialized, json);
    }
}
