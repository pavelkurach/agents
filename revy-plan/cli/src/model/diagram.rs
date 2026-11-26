//! The 16 authored diagram kinds a plan's narrative can embed via
//! [`crate::model::content::Block::Figure`].
//!
//! [`Diagram`] is a closed sum over every kind `presentation.md` documents:
//! `layers`, `flow`, `seq`, `er`, `endpoint`, `dag`, `states`, `layout`,
//! `timeline`, `tree`, `matrix`, `trace`, `budget`, `compare`, `stack`,
//! `examples`. Every
//! substruct reuses the crate's existing primitives (`NonEmptyText`, `Slug`,
//! `RepoRelPath`, `OneOrMore`) — no new primitive types, no free-form maps, no
//! stringly shorthand payloads. See
//! `.superpowers/sdd/2026-08-14-revy-plan-evidence-first-simplification/phase2-R2-design.md`
//! for the full rationale per type.
//!
//! Deliberate deviations from that design document, each noted at its
//! definition below:
//! - [`FlowOrientation`] is nested under a `flow` field rather than
//!   `#[serde(flatten)]`ed directly onto [`Diagram::Flow`] — flatten over an
//!   internally tagged enum combined with `deny_unknown_fields` is a confirmed
//!   serde conflict (it rejects even valid input); nesting one level nested
//!   avoids it while keeping the choice closed, typed, and parse-safe.
//! - `Matrix`'s column-reference validation (§0.3) lands in R2c, alongside
//!   its renderer, and hits the same `try_from`-cannot-target-a-single-
//!   variant limitation `Seq` documents below: [`Diagram::Matrix`] carries a
//!   `#[serde(flatten)]`ed [`MatrixBody`], which itself carries
//!   `#[serde(try_from = "RawMatrixDiagram")]`. Unlike `Seq`, no field
//!   changes shape between raw and validated form (`MatrixCell.column` is
//!   already a plain name on the wire), so `MatrixBody` needs no
//!   `#[serde(into = ...)]` projection — see its own doc comment.
//! - `Seq`'s actor-reference validation (§0.3) is real, but not shaped
//!   exactly as the design's pseudocode: `#[serde(try_from = "Raw...")]` is a
//!   *container*-level attribute and cannot target a single enum variant
//!   directly (confirmed serde limitation — R2a's commit deferred this here
//!   for exactly that reason). [`Diagram::Seq`] instead carries one
//!   `#[serde(flatten)]`ed field of type [`SeqBody`], which itself carries
//!   the `#[serde(try_from = "RawSeqDiagram")]` attribute; flattening a plain
//!   struct (not a further tagged enum, unlike `Flow`'s case above) into a
//!   `deny_unknown_fields` variant is not the same serde limitation and was
//!   verified empirically to compose correctly with both `deny_unknown_fields`
//!   and `TryFrom`-based rejection. The wire JSON stays exactly as flat as
//!   every other variant: `{"kind":"seq","actors":[...],"steps":[...],"spans":[...]}`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::primitives::{NonEmptyText, OneOrMore, RepoPath, Slug};

// --- Shared: `Crossing` (§3) ----------------------------------------------------

/// One labelled crossing between two named things — shared by `Layers.edges`
/// and `Dag.edges`.
///
/// `absent` replaces the old schema's one-value `kind: none` (a size-1
/// vocabulary is a `bool`, not a one-variant enum, per §0 rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Crossing {
    pub from: NonEmptyText,
    pub to: NonEmptyText,
    pub label: Option<NonEmptyText>,
    pub note: Option<NonEmptyText>,
    pub absent: bool,
}

// --- 1. `Layers` (real renderer) ------------------------------------------------

/// One horizontal tier, top to bottom, and the components inside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    pub name: NonEmptyText,
    pub nodes: OneOrMore<LayerNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerNode {
    pub name: NonEmptyText,
    pub kind: Option<LayerNodeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayerNodeKind {
    New,
    External,
}

// --- 2. `Flow` (real renderer) --------------------------------------------------

/// A pipeline (`Horizontal`) or a path with every early exit drawn
/// (`Vertical`). Nested under [`Diagram::Flow`]'s `flow` field rather than
/// flattened — see the module doc comment for why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "orientation", rename_all = "snake_case", deny_unknown_fields)]
pub enum FlowOrientation {
    Horizontal { steps: OneOrMore<FlowStep> },
    Vertical { steps: OneOrMore<VerticalStep> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowStep {
    pub name: NonEmptyText,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerticalStep {
    pub name: NonEmptyText,
    pub note: Option<NonEmptyText>,
    pub terminal: bool,
    #[serde(default)]
    pub outcomes: Vec<Outcome>,
}

/// An early exit from a vertical flow step. `kind` is required (not defaulted
/// to `error`, the old renderer's silent default, `render.py:308`) — see §0.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub when: NonEmptyText,
    pub then: NonEmptyText,
    pub kind: OutcomeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    Error,
    Exit,
}

// --- 3. `Seq` (real renderer, R2b) ----------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub name: NonEmptyText,
    pub note: Option<NonEmptyText>,
}

/// A validated reference into `Seq.actors`.
///
/// An in-bounds 0-based index, resolved and range-checked at parse time by
/// [`RawSeqDiagram`]'s `TryFrom` — the direct typed replacement for
/// `render.py:346-349`'s runtime `ValueError`; see the module doc comment and
/// design §0.3. No public constructor: the only way to produce one is
/// successful resolution against a declared actor.
///
/// Serializes as its bare 0-based position — matching `spans[].from/to`'s
/// wire shape, which is correct and left as-is. `steps[].from/to`'s wire
/// shape is name-based instead ([`RawStep`]'s `from`/`to` only accept a
/// declared actor *name*), so [`SeqBody`] projects `Step`'s `ActorIndex`
/// fields back into names at serialize time via its own
/// `#[serde(into = "WireSeqDiagram")]`, rather than by changing this type —
/// see [`WireSeqDiagram`]. `ActorIndex` itself, and `Span`'s serialization,
/// are untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct ActorIndex(usize);

impl ActorIndex {
    /// This reference's 0-based position in `Seq.actors` — used at render
    /// time to compute the actor's column (`render/diagram/seq.rs`).
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// A message between two actors. `from`/`to` are resolved [`ActorIndex`]es —
/// see the module doc comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Step {
    pub from: ActorIndex,
    pub to: ActorIndex,
    pub label: NonEmptyText,
    pub kind: Option<StepKind>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Return,
    #[serde(rename = "self")]
    SelfCall,
    Error,
}

/// A boundary drawn across actor columns. `from`/`to` are resolved
/// [`ActorIndex`]es — see the module doc comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Span {
    pub from: ActorIndex,
    pub to: ActorIndex,
    pub label: NonEmptyText,
    pub kind: Option<SpanKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    Tx,
    Lock,
}

/// A step's raw, unvalidated actor reference — see [`RawSeqDiagram`].
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawStep {
    from: NonEmptyText,
    to: NonEmptyText,
    label: NonEmptyText,
    kind: Option<StepKind>,
    note: Option<NonEmptyText>,
}

/// A span's raw, unvalidated actor reference — a 0-based position, per old
/// semantics (`presentation.md:236`), not a name; see [`RawSeqDiagram`].
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawSpan {
    from: usize,
    to: usize,
    label: NonEmptyText,
    kind: Option<SpanKind>,
}

/// The author-facing wire shape of [`Diagram::Seq`]: `steps[].from/to` are
/// actor *names* (resolved against `actors` by declared `name`);
/// `spans[].from/to` are 0-based *positions* (old semantics,
/// `presentation.md:236`). Deserialized first, then converted via `TryFrom`
/// into [`SeqBody`] — see the module doc comment and design §0.3.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawSeqDiagram {
    actors: OneOrMore<Actor>,
    #[serde(default)]
    steps: Vec<RawStep>,
    #[serde(default)]
    spans: Vec<RawSpan>,
}

/// Why a `Seq` diagram's actor references failed to resolve.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SeqRefError {
    #[error("step references actor `{0}`, which is not declared in `actors`")]
    UndeclaredActor(String),
    #[error("span position {position} is out of range for {actor_count} declared actor(s)")]
    SpanOutOfRange { position: usize, actor_count: usize },
}

/// [`Diagram::Seq`]'s payload, resolved and bounds-checked.
///
/// Every actor reference is an [`ActorIndex`] by the time this exists.
/// Flattened into `Diagram::Seq` so the wire JSON stays flat even though the
/// `TryFrom`-based cross validation must route through this nested Rust type
/// — see the module doc comment.
///
/// `#[serde(into = "WireSeqDiagram")]` coexists with `try_from` below: they
/// govern opposite directions (`try_from` replaces the generated
/// `Deserialize`, `into` replaces the generated `Serialize`) and serde
/// derives them independently, so both compose on the same container. This
/// closes the round-trip gap `ActorIndex` used to document: `steps[].from/to`
/// now serialize back to the declared actor *name* [`RawStep`] expects,
/// instead of `ActorIndex`'s own bare-position wire shape — see
/// [`WireSeqDiagram`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RawSeqDiagram", into = "WireSeqDiagram")]
pub struct SeqBody {
    pub actors: OneOrMore<Actor>,
    pub steps: Vec<Step>,
    pub spans: Vec<Span>,
}

impl TryFrom<RawSeqDiagram> for SeqBody {
    type Error = SeqRefError;

    fn try_from(raw: RawSeqDiagram) -> Result<Self, Self::Error> {
        let actor_count = raw.actors.as_slice().len();
        let resolve_name = |name: &NonEmptyText| -> Result<ActorIndex, SeqRefError> {
            raw.actors
                .as_slice()
                .iter()
                .position(|actor| actor.name.as_str() == name.as_str())
                .map(ActorIndex)
                .ok_or_else(|| SeqRefError::UndeclaredActor(name.as_str().to_owned()))
        };
        let resolve_position = |position: usize| -> Result<ActorIndex, SeqRefError> {
            if position < actor_count {
                Ok(ActorIndex(position))
            } else {
                Err(SeqRefError::SpanOutOfRange {
                    position,
                    actor_count,
                })
            }
        };

        let steps = raw
            .steps
            .iter()
            .map(|step| {
                Ok(Step {
                    from: resolve_name(&step.from)?,
                    to: resolve_name(&step.to)?,
                    label: step.label.clone(),
                    kind: step.kind,
                    note: step.note.clone(),
                })
            })
            .collect::<Result<Vec<_>, SeqRefError>>()?;

        let spans = raw
            .spans
            .iter()
            .map(|span| {
                Ok(Span {
                    from: resolve_position(span.from)?,
                    to: resolve_position(span.to)?,
                    label: span.label.clone(),
                    kind: span.kind,
                })
            })
            .collect::<Result<Vec<_>, SeqRefError>>()?;

        Ok(Self {
            actors: raw.actors,
            steps,
            spans,
        })
    }
}

/// [`SeqBody`]'s serialize-only wire shape — the mirror image of
/// [`RawSeqDiagram`]: `steps[].from/to` are the referenced actor's declared
/// *name*, resolved by position against `actors`, matching what [`RawStep`]
/// accepts on the way in. `spans[].from/to` reuse [`Span`] unchanged: its
/// `ActorIndex` fields already serialize as the 0-based position [`RawSpan`]
/// expects, so no separate wire type is needed there. Exists solely so
/// `serde_json::to_string(&diagram)` followed by `serde_json::from_str`
/// reproduces the original `Diagram::Seq` — see the module doc comment and
/// [`SeqBody`]'s `#[serde(into = ...)]`.
#[derive(Debug, Serialize, JsonSchema)]
struct WireStep {
    from: NonEmptyText,
    to: NonEmptyText,
    label: NonEmptyText,
    kind: Option<StepKind>,
    note: Option<NonEmptyText>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct WireSeqDiagram {
    actors: OneOrMore<Actor>,
    steps: Vec<WireStep>,
    spans: Vec<Span>,
}

impl From<SeqBody> for WireSeqDiagram {
    fn from(body: SeqBody) -> Self {
        let steps = body
            .steps
            .iter()
            .map(|step| WireStep {
                from: resolve_actor_name(&body.actors, step.from),
                to: resolve_actor_name(&body.actors, step.to),
                label: step.label.clone(),
                kind: step.kind,
                note: step.note.clone(),
            })
            .collect();

        Self {
            actors: body.actors,
            steps,
            spans: body.spans,
        }
    }
}

/// Looks up the declared actor name at `index` — the serialize-time mirror of
/// `TryFrom<RawSeqDiagram>`'s `resolve_name` above. `index` is always in
/// bounds in practice: the only way an `ActorIndex` comes to exist at all is
/// successful resolution against this same `actors` list, which `SeqBody`
/// carries unchanged from `RawSeqDiagram`. Still written as a total function
/// rather than a direct index (`clippy::indexing_slicing` is denied
/// crate-wide, and the module forbids panics in non-test code): an
/// out-of-bounds lookup falls back to a placeholder name instead.
fn resolve_actor_name(actors: &OneOrMore<Actor>, index: ActorIndex) -> NonEmptyText {
    actors.as_slice().get(index.as_usize()).map_or_else(
        || NonEmptyText::from_static("<invalid actor index>"),
        |actor| actor.name.clone(),
    )
}

// --- 4. `Er` (real renderer, R2c) ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entity {
    pub name: NonEmptyText,
    pub kind: Option<EntityKind>,
    pub fields: OneOrMore<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    New,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: NonEmptyText,
    #[serde(rename = "type")]
    pub type_name: NonEmptyText,
    pub key: Option<FieldKey>,
}

/// A field's key role. Closed over `pk:true` / `fk:<entity>.<field>`, making
/// "both pk and fk" and "fk with no target" unrepresentable — see §0.5.
///
/// `Primary` is `Primary {}` (an empty struct variant), not a bare unit
/// variant: serde's internally tagged representation does not enforce
/// `deny_unknown_fields` against a unit variant (confirmed empirically — it
/// silently ignores any extra fields alongside the tag), but does enforce it
/// against an empty struct variant. `{"kind":"primary"}` still parses either
/// way; only the "both pk and fk" rejection depends on this shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FieldKey {
    Primary {},
    Foreign { references: NonEmptyText },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub from: NonEmptyText,
    pub to: NonEmptyText,
    pub kind: RelationKind,
    pub label: Option<NonEmptyText>,
}

/// A relation's cardinality. Closed so there is no YAML scalar-typing
/// ambiguity to quote around (the old schema's `n-1` footgun, `presentation.md:255`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    OneToMany,
    ManyToOne,
    OneToOne,
}

// --- 5. `Endpoint` (real renderer, R2c) -----------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

/// Whether the endpoint requires auth, and why not when it doesn't — `auth`
/// is a required field on [`Diagram::Endpoint`] precisely so this choice
/// cannot be silently omitted; see §0.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Auth {
    Open { reason: NonEmptyText },
    Scheme { name: NonEmptyText },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub name: NonEmptyText,
    #[serde(rename = "in")]
    pub location: ParamLocation,
    #[serde(rename = "type")]
    pub type_name: NonEmptyText,
    pub required: bool,
    pub description: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ParamLocation {
    Path,
    Query,
    Header,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub content_type: Option<NonEmptyText>,
    pub example: Option<NonEmptyText>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub status: u16,
    pub description: NonEmptyText,
    pub kind: Option<ResponseKind>,
    #[serde(default)]
    pub headers: Vec<NonEmptyText>,
    pub body: Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    New,
    Changed,
}

// --- 6. `Dag` (real renderer, R2b) ----------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wave {
    pub name: Option<NonEmptyText>,
    pub mrs: OneOrMore<WaveMr>,
    pub gate: Option<NonEmptyText>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaveMr {
    pub id: Slug,
    pub title: NonEmptyText,
    pub kind: Option<MrKind>,
    pub flag: Option<NonEmptyText>,
    pub run: Option<NonEmptyText>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MrKind {
    Migration,
    Oneway,
    Scaffolding,
}

// --- 7. `States` (real renderer) ------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub name: NonEmptyText,
    pub kind: Option<StateKind>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateKind {
    Initial,
    Terminal,
    Error,
    New,
}

/// A state transition.
///
/// `from`/`to` are **not** validated against declared `states` — old Python
/// didn't either (`render.py:593-620`); adding that check would be scope
/// creep beyond what the schema documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub from: NonEmptyText,
    pub to: NonEmptyText,
    pub on: Option<NonEmptyText>,
    pub kind: Option<TransitionKind>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Error,
    New,
}

// --- 8. `Layout` (real renderer, R2b) -------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutField {
    pub name: NonEmptyText,
    pub size: std::num::NonZeroU32,
    pub kind: Option<LayoutFieldKind>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayoutFieldKind {
    New,
    Reserved,
    Removed,
}

// --- 9. `Timeline` (real renderer) ----------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimelineStep {
    pub name: NonEmptyText,
    pub kind: Option<TimelineStepKind>,
    pub wait: Option<NonEmptyText>,
    pub rollback: Option<NonEmptyText>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimelineStepKind {
    Gate,
    Oneway,
}

// --- 10. `Tree` (real renderer, R2b) --------------------------------------------

/// A file/config hierarchy row.
///
/// **Deliberate deviation from old Python**: every entry names a concrete
/// file — [`RepoPath`] rejects a trailing `/`, so a directory-only row
/// (old `render.py:757`) is unrepresentable. Reusing the canonical repo-path
/// primitive outweighs keeping that minor display nuance; see §1 item 10.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TreeEntry {
    pub path: RepoPath,
    pub change: Option<TreeChangeKind>,
    pub note: Option<NonEmptyText>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TreeChangeKind {
    Added,
    Modified,
    Removed,
}

// --- 11. `Matrix` (real renderer + column `TryFrom`, R2c) -----------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatrixRow {
    pub name: NonEmptyText,
    #[serde(default)]
    pub cells: Vec<MatrixCell>,
}

/// One declared column's value for a row.
///
/// `column` is a `NonEmptyText` name, not an index — see [`MatrixBody`]'s
/// doc comment for why that keeps this type's wire shape symmetric on the
/// way back out, unlike `Seq`'s `ActorIndex`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MatrixCell {
    pub column: NonEmptyText,
    pub value: NonEmptyText,
    pub kind: Option<MatrixCellKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MatrixCellKind {
    Differs,
    New,
    Risk,
}

/// [`Diagram::Matrix`]'s raw, unvalidated wire shape — see [`MatrixBody`].
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RawMatrixDiagram {
    columns: OneOrMore<NonEmptyText>,
    rows: OneOrMore<MatrixRow>,
}

/// Why a `Matrix` diagram's cell columns failed to validate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MatrixColumnError {
    #[error("row `{row}` has a cell for column `{column}`, which is not declared in `columns`")]
    UndeclaredColumn { row: String, column: String },
    #[error("row `{row}` has more than one cell for column `{column}`")]
    DuplicateColumn { row: String, column: String },
}

/// [`Diagram::Matrix`]'s payload, validated.
///
/// Every `cells[].column` is declared in `columns`, and no row repeats a
/// `column` — the typed replacement for `render.py:779`'s silent
/// dict-lookup-on-a-typo (design §0.3/§1.11).
///
/// Flattened into `Diagram::Matrix` rather than typed directly on the enum
/// variant — the same serde limitation [`SeqBody`]'s doc comment explains:
/// `#[serde(try_from = ...)]` is a container-level attribute and cannot
/// target a single enum variant directly. Unlike `SeqBody`, no field changes
/// shape between the raw and validated form here — `MatrixCell.column` is
/// already a plain name on both sides of the wire — so this type needs no
/// `#[serde(into = ...)]` projection: the derived `Serialize` alone
/// round-trips through [`RawMatrixDiagram`] as-is (proved by
/// `test_matrix_diagram_round_trips_through_a_serialized_json_string`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RawMatrixDiagram")]
pub struct MatrixBody {
    pub columns: OneOrMore<NonEmptyText>,
    pub rows: OneOrMore<MatrixRow>,
}

impl TryFrom<RawMatrixDiagram> for MatrixBody {
    type Error = MatrixColumnError;

    fn try_from(raw: RawMatrixDiagram) -> Result<Self, Self::Error> {
        for row in raw.rows.as_slice() {
            let mut seen_columns: Vec<&str> = Vec::new();
            for cell in &row.cells {
                let column = cell.column.as_str();
                if !raw
                    .columns
                    .as_slice()
                    .iter()
                    .any(|declared| declared.as_str() == column)
                {
                    return Err(MatrixColumnError::UndeclaredColumn {
                        row: row.name.as_str().to_owned(),
                        column: column.to_owned(),
                    });
                }
                if seen_columns.contains(&column) {
                    return Err(MatrixColumnError::DuplicateColumn {
                        row: row.name.as_str().to_owned(),
                        column: column.to_owned(),
                    });
                }
                seen_columns.push(column);
            }
        }

        Ok(Self {
            columns: raw.columns,
            rows: raw.rows,
        })
    }
}

// --- 12. `Trace` (real renderer, R2b) -------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LegendEntry {
    pub symbol: NonEmptyText,
    pub meaning: NonEmptyText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceOp {
    pub op: NonEmptyText,
    pub key: NonEmptyText,
    pub kind: Option<TraceOpKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TraceOpKind {
    Waste,
    Saved,
    New,
}

// --- 13. `Budget` (real renderer, R2b) ------------------------------------------

// `value` is `f64` (§0.4: numeric fields use plain stdlib numeric types, with
// any non-negativity/scale invariant enforced defensively at render time, not
// parse time — a negative or zero budget value is a legitimate authored fact).
// `f64` has no `Eq`, so it is asserted manually below rather than derived —
// `Eq` is a marker trait with no required methods; the plan never compares
// budget parts for NaN-sensitive equality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BudgetPart {
    pub name: NonEmptyText,
    pub value: f64,
    pub kind: Option<BudgetPartKind>,
}

impl Eq for BudgetPart {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPartKind {
    New,
    Risk,
}

// --- 14. `Compare` (real renderer, R2c) -----------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareColumn {
    pub label: NonEmptyText,
    pub items: OneOrMore<CompareItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompareItem {
    pub text: NonEmptyText,
    pub kind: Option<CompareItemKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompareItemKind {
    Add,
    Del,
}

// --- 15. `Stack` (real renderer, R2c) -------------------------------------------

/// A call-tree frame.
///
/// `children` being recursive replaces the old text grammar's 2-space indent
/// counting entirely (`as_stack`, `render.py:202-231`) — that authoring
/// channel does not exist anymore, see §1 item 15.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StackFrame {
    pub marker: Option<StackMarker>,
    pub symbol: NonEmptyText,
    pub note: Option<NonEmptyText>,
    #[serde(default)]
    pub children: Vec<Self>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StackMarker {
    Added,
    Removed,
    Changed,
    Called,
}

// --- 16. `Examples` (worked given/when/then rows) ------------------------------

/// How a worked example reads, which colours its row. Closed: an example shows a
/// normal case, a boundary/edge case, or a rejected/error case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExampleKind {
    Wire,
    Boundary,
    Error,
}

/// One worked example: a prior state, an action, and the observable result.
///
/// `LPUSH k a b c` → `c,b,a` is one row; `given` carries the starting state only
/// when the result depends on it. Concrete input→output pairs render as a
/// scannable table instead of a comma-pile buried in prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkedExample {
    pub given: Option<NonEmptyText>,
    pub when: NonEmptyText,
    pub then: NonEmptyText,
    pub kind: Option<ExampleKind>,
}

// --- The closed 16-variant sum -------------------------------------------------

/// One authored diagram, embedded in the plan narrative via
/// [`crate::model::content::Block::Figure`].
///
/// Closed: every kind `presentation.md` documents has a variant here, and
/// every one of the 16 has a real renderer — see `render/diagram`.
///
/// `Eq` is asserted manually (below), not derived: the `Budget` variant
/// carries a bare `f64`, which has no `Eq` — same rationale as [`BudgetPart`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Diagram {
    Layers {
        layers: OneOrMore<Tier>,
        #[serde(default)]
        edges: Vec<Crossing>,
    },
    Flow {
        flow: FlowOrientation,
    },
    Seq {
        #[serde(flatten)]
        body: SeqBody,
    },
    Er {
        entities: OneOrMore<Entity>,
        #[serde(default)]
        relations: Vec<Relation>,
    },
    Endpoint {
        method: HttpMethod,
        path: NonEmptyText,
        summary: Option<NonEmptyText>,
        auth: Auth,
        #[serde(default)]
        params: Vec<Param>,
        request: Option<Body>,
        responses: OneOrMore<Response>,
    },
    Dag {
        caption: Option<NonEmptyText>,
        waves: OneOrMore<Wave>,
        #[serde(default)]
        edges: Vec<Crossing>,
    },
    States {
        states: OneOrMore<State>,
        #[serde(default)]
        transitions: Vec<Transition>,
    },
    Layout {
        caption: Option<NonEmptyText>,
        unit: NonEmptyText,
        fields: OneOrMore<LayoutField>,
    },
    Timeline {
        steps: OneOrMore<TimelineStep>,
    },
    Tree {
        entries: OneOrMore<TreeEntry>,
    },
    Matrix {
        #[serde(flatten)]
        body: MatrixBody,
    },
    Trace {
        caption: Option<NonEmptyText>,
        #[serde(default)]
        legend: Vec<LegendEntry>,
        ops: OneOrMore<TraceOp>,
    },
    Budget {
        budget: f64,
        unit: NonEmptyText,
        parts: OneOrMore<BudgetPart>,
    },
    Compare {
        columns: OneOrMore<CompareColumn>,
    },
    Stack {
        frames: OneOrMore<StackFrame>,
    },
    Examples {
        caption: Option<NonEmptyText>,
        rows: OneOrMore<WorkedExample>,
    },
}

impl Eq for Diagram {}

impl Diagram {
    /// The `kind` this diagram renders as — used for the Markdown textual
    /// fallback (`render/diagram`).
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Layers { .. } => "layers",
            Self::Flow { .. } => "flow",
            Self::Seq { .. } => "seq",
            Self::Er { .. } => "er",
            Self::Endpoint { .. } => "endpoint",
            Self::Dag { .. } => "dag",
            Self::States { .. } => "states",
            Self::Layout { .. } => "layout",
            Self::Timeline { .. } => "timeline",
            Self::Tree { .. } => "tree",
            Self::Matrix { .. } => "matrix",
            Self::Trace { .. } => "trace",
            Self::Budget { .. } => "budget",
            Self::Compare { .. } => "compare",
            Self::Stack { .. } => "stack",
            Self::Examples { .. } => "examples",
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::too_many_lines
    )]
    use super::*;

    // --- Layers: parse-reject + parse-smoke ------------------------------------

    #[test]
    fn test_layers_rejects_a_tier_with_zero_nodes() {
        let json = serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": []}],
            "edges": []
        });

        serde_json::from_value::<Diagram>(json).expect_err("empty nodes must be rejected");
    }

    #[test]
    fn test_layers_rejects_an_unknown_field() {
        let json = serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": null}]}],
            "edges": [],
            "bogus": true
        });

        serde_json::from_value::<Diagram>(json).expect_err("unknown field must be rejected");
    }

    #[test]
    fn test_layers_parses_a_minimal_valid_document() {
        let json = serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": "new"}]}],
            "edges": [{"from": "api", "to": "db", "label": null, "note": null, "absent": false}]
        });

        serde_json::from_value::<Diagram>(json).expect("valid layers document parses");
    }

    // --- Flow: the flagged serde risk (§ known conflict) -----------------------

    #[test]
    fn test_flow_parses_a_valid_horizontal_document() {
        let json = serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "horizontal",
                "steps": [{"name": "Receive request", "note": null}]
            }
        });

        let diagram: Diagram = serde_json::from_value(json).expect("valid horizontal flow parses");
        assert!(matches!(
            diagram,
            Diagram::Flow {
                flow: FlowOrientation::Horizontal { .. }
            }
        ));
    }

    #[test]
    fn test_flow_parses_a_valid_vertical_document() {
        let json = serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{
                    "name": "Validate",
                    "note": null,
                    "terminal": false,
                    "outcomes": [{"when": "invalid", "then": "400", "kind": "error"}]
                }]
            }
        });

        let diagram: Diagram = serde_json::from_value(json).expect("valid vertical flow parses");
        assert!(matches!(
            diagram,
            Diagram::Flow {
                flow: FlowOrientation::Vertical { .. }
            }
        ));
    }

    #[test]
    fn test_flow_rejects_an_unknown_field() {
        let json = serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "horizontal",
                "steps": [{"name": "Receive request", "note": null}]
            },
            "bogus": true
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("unknown top-level field must be rejected");
    }

    #[test]
    fn test_flow_rejects_an_unknown_field_inside_the_orientation() {
        let json = serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "horizontal",
                "steps": [{"name": "Receive request", "note": null}],
                "bogus": true
            }
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("unknown field inside the orientation must be rejected");
    }

    #[test]
    fn test_flow_vertical_outcome_requires_kind() {
        let json = serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{
                    "name": "Validate",
                    "note": null,
                    "terminal": false,
                    "outcomes": [{"when": "invalid", "then": "400"}]
                }]
            }
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("a vertical outcome missing `kind` must be rejected, not defaulted");
    }

    // --- States: parse-reject + parse-smoke -------------------------------------

    #[test]
    fn test_states_rejects_a_transition_with_an_unknown_kind() {
        let json = serde_json::json!({
            "kind": "states",
            "states": [{"name": "Idle", "kind": "initial", "note": null}],
            "transitions": [{"from": "Idle", "to": "Running", "on": null, "kind": "bogus", "note": null}]
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("an unknown transition kind must be rejected");
    }

    #[test]
    fn test_states_parses_a_minimal_valid_document() {
        let json = serde_json::json!({
            "kind": "states",
            "states": [{"name": "Idle", "kind": null, "note": null}],
            "transitions": []
        });

        serde_json::from_value::<Diagram>(json).expect("valid states document parses");
    }

    // --- Timeline: parse-reject + parse-smoke -----------------------------------

    #[test]
    fn test_timeline_rejects_zero_steps() {
        let json = serde_json::json!({"kind": "timeline", "steps": []});

        serde_json::from_value::<Diagram>(json).expect_err("empty steps must be rejected");
    }

    #[test]
    fn test_timeline_parses_a_minimal_valid_document() {
        let json = serde_json::json!({
            "kind": "timeline",
            "steps": [{"name": "Deploy", "kind": null, "wait": null, "rollback": null, "note": null}]
        });

        serde_json::from_value::<Diagram>(json).expect("valid timeline document parses");
    }

    // --- Er: unrepresentable pk+fk -----------------------------------------------

    #[test]
    fn test_field_key_cannot_carry_both_primary_and_foreign_facts() {
        // `references` only exists on `Foreign`; supplying it against `kind:
        // primary` is an unknown field on that variant, not a valid "both" state.
        let json = serde_json::json!({"kind": "primary", "references": "orders.id"});

        serde_json::from_value::<FieldKey>(json)
            .expect_err("a primary field key cannot also carry a foreign reference");
    }

    // --- Seq: `TryFrom`-based actor-reference validation (§0.3) -----------------

    fn seq_json(
        actors: &serde_json::Value,
        steps: &serde_json::Value,
        spans: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::json!({"kind": "seq", "actors": actors, "steps": steps, "spans": spans})
    }

    #[test]
    fn test_seq_rejects_a_step_naming_an_undeclared_actor() {
        let json = seq_json(
            &serde_json::json!([{"name": "Client", "note": null}]),
            &serde_json::json!([{"from": "Client", "to": "Ghost", "label": "req", "kind": null, "note": null}]),
            &serde_json::json!([]),
        );

        let error =
            serde_json::from_value::<Diagram>(json).expect_err("undeclared actor must be rejected");
        assert!(
            error.to_string().contains("Ghost"),
            "error must name the undeclared actor: {error}"
        );
    }

    #[test]
    fn test_seq_rejects_a_span_position_out_of_range() {
        let json = seq_json(
            &serde_json::json!([{"name": "Client", "note": null}]),
            &serde_json::json!([]),
            &serde_json::json!([{"from": 0, "to": 5, "label": "tx", "kind": null}]),
        );

        serde_json::from_value::<Diagram>(json)
            .expect_err("an out-of-range span position must be rejected");
    }

    #[test]
    fn test_seq_rejects_an_unknown_top_level_field() {
        let mut json = seq_json(
            &serde_json::json!([{"name": "Client", "note": null}]),
            &serde_json::json!([]),
            &serde_json::json!([]),
        );
        json["bogus"] = serde_json::json!(true);

        serde_json::from_value::<Diagram>(json).expect_err("unknown field must be rejected");
    }

    #[test]
    fn test_seq_resolves_a_declared_actor_name_to_its_position() {
        let json = seq_json(
            &serde_json::json!([{"name": "Client", "note": null}, {"name": "Server", "note": null}]),
            &serde_json::json!([{"from": "Client", "to": "Server", "label": "req", "kind": null, "note": null}]),
            &serde_json::json!([]),
        );

        let diagram: Diagram = serde_json::from_value(json).expect("valid seq document parses");
        let Diagram::Seq { body } = diagram else {
            panic!("expected a Seq diagram");
        };
        assert_eq!(body.steps[0].from.as_usize(), 0);
        assert_eq!(body.steps[0].to.as_usize(), 1);
    }

    #[test]
    fn test_seq_diagram_round_trips_through_a_serialized_json_string() {
        // Regression test for the bug this module's `WireSeqDiagram` fixes:
        // `ActorIndex` serializes as a bare position, but `RawStep.from/to`
        // only deserializes from a declared actor *name* — without the
        // `#[serde(into = "WireSeqDiagram")]` projection, re-parsing this
        // diagram's own `to_string()` output fails with `invalid type:
        // integer, expected a string`.
        let json = seq_json(
            &serde_json::json!([
                {"name": "Client", "note": null},
                {"name": "Server", "note": "primary replica"}
            ]),
            &serde_json::json!([
                {"from": "Client", "to": "Server", "label": "req", "kind": null, "note": null},
                {"from": "Server", "to": "Client", "label": "200 OK", "kind": "return", "note": null}
            ]),
            &serde_json::json!([
                {"from": 0, "to": 1, "label": "tx", "kind": "tx"}
            ]),
        );

        let diagram: Diagram = serde_json::from_value(json).expect("valid seq document parses");
        let serialized = serde_json::to_string(&diagram).expect("seq diagram serializes");
        let reparsed: Diagram = serde_json::from_str(&serialized).unwrap_or_else(|error| {
            panic!("re-parsing a serialized seq diagram must succeed: {error}\n{serialized}")
        });

        assert_eq!(diagram, reparsed);
    }

    // --- Matrix: `TryFrom`-based column validation (§0.3, R2c) ------------------

    fn matrix_json(columns: &serde_json::Value, rows: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({"kind": "matrix", "columns": columns, "rows": rows})
    }

    #[test]
    fn test_matrix_rejects_a_cell_column_not_declared() {
        let json = matrix_json(
            &serde_json::json!(["dev", "prod"]),
            &serde_json::json!([{
                "name": "timeout",
                "cells": [{"column": "staging", "value": "30s", "kind": null}]
            }]),
        );

        let error = serde_json::from_value::<Diagram>(json)
            .expect_err("a cell column not declared in `columns` must be rejected");
        assert!(
            error.to_string().contains("staging"),
            "error must name the undeclared column: {error}"
        );
    }

    #[test]
    fn test_matrix_rejects_a_duplicate_column_within_one_row() {
        let json = matrix_json(
            &serde_json::json!(["dev", "prod"]),
            &serde_json::json!([{
                "name": "timeout",
                "cells": [
                    {"column": "prod", "value": "30s", "kind": null},
                    {"column": "prod", "value": "45s", "kind": null}
                ]
            }]),
        );

        serde_json::from_value::<Diagram>(json)
            .expect_err("a duplicate column within one row must be rejected");
    }

    #[test]
    fn test_matrix_accepts_a_row_that_omits_a_declared_column() {
        // Design §1.11: an unmarked cell is context, not an error — only a
        // *present* cell naming an undeclared column is rejected.
        let json = matrix_json(
            &serde_json::json!(["dev", "prod"]),
            &serde_json::json!([{"name": "timeout", "cells": []}]),
        );

        serde_json::from_value::<Diagram>(json).expect("a row with no cells is valid");
    }

    #[test]
    fn test_matrix_diagram_round_trips_through_a_serialized_json_string() {
        // `MatrixCell.column` is a name, not an index (unlike `Seq`'s
        // `ActorIndex`), so this is a plain regression guard rather than a
        // fix for a known wire-shape asymmetry — see the module doc comment.
        let json = matrix_json(
            &serde_json::json!(["dev", "prod"]),
            &serde_json::json!([{
                "name": "timeout",
                "cells": [
                    {"column": "dev", "value": "5s", "kind": "new"},
                    {"column": "prod", "value": "30s", "kind": "risk"}
                ]
            }]),
        );

        let diagram: Diagram = serde_json::from_value(json).expect("valid matrix document parses");
        let serialized = serde_json::to_string(&diagram).expect("matrix diagram serializes");
        let reparsed: Diagram = serde_json::from_str(&serialized).unwrap_or_else(|error| {
            panic!("re-parsing a serialized matrix diagram must succeed: {error}\n{serialized}")
        });

        assert_eq!(diagram, reparsed);
    }

    // --- Examples (worked given/when/then rows) ---------------------------------

    #[test]
    fn test_examples_rejects_an_empty_when() {
        let json = serde_json::json!({
            "kind": "examples",
            "caption": null,
            "rows": [{"given": null, "when": "", "then": "c,b,a", "kind": null}]
        });

        serde_json::from_value::<Diagram>(json).expect_err("an empty `when` must be rejected");
    }

    #[test]
    fn test_examples_rejects_an_empty_then() {
        let json = serde_json::json!({
            "kind": "examples",
            "caption": null,
            "rows": [{"given": null, "when": "LPUSH k a", "then": "", "kind": null}]
        });

        serde_json::from_value::<Diagram>(json).expect_err("an empty `then` must be rejected");
    }

    #[test]
    fn test_examples_rejects_an_empty_rows_array() {
        let json = serde_json::json!({"kind": "examples", "caption": null, "rows": []});

        serde_json::from_value::<Diagram>(json).expect_err("an empty rows array must be rejected");
    }

    #[test]
    fn test_examples_rejects_an_unknown_field() {
        let json = serde_json::json!({
            "kind": "examples",
            "caption": null,
            "rows": [{"given": null, "when": "LPUSH k a", "then": "a", "kind": null, "extra": true}]
        });

        serde_json::from_value::<Diagram>(json).expect_err("an unknown field must be rejected");
    }

    #[test]
    fn test_examples_diagram_round_trips_through_a_serialized_json_string() {
        let json = serde_json::json!({
            "kind": "examples",
            "caption": "List commands",
            "rows": [
                {"given": "k = a,b", "when": "LPUSH k c d", "then": "k = d,c,a,b", "kind": "wire"},
                {"given": null, "when": "LPOP missing", "then": "$-1\r\n", "kind": "boundary"},
                {"given": null, "when": "LPUSH k", "then": "arity error", "kind": "error"}
            ]
        });

        let diagram: Diagram =
            serde_json::from_value(json).expect("valid examples document parses");
        let serialized = serde_json::to_string(&diagram).expect("examples diagram serializes");
        let reparsed: Diagram = serde_json::from_str(&serialized).unwrap_or_else(|error| {
            panic!("re-parsing a serialized examples diagram must succeed: {error}\n{serialized}")
        });

        assert_eq!(diagram, reparsed);
    }

    // --- Er/Endpoint/Matrix/Compare/Stack: parse-smoke + round-trip (R2c) -------

    fn minimal_valid_json_for_each_deferred_kind() -> Vec<(&'static str, serde_json::Value)> {
        vec![
            (
                "er",
                serde_json::json!({
                    "kind": "er",
                    "entities": [{
                        "name": "Order",
                        "kind": null,
                        "fields": [{"name": "id", "type": "uuid", "key": {"kind": "primary"}}]
                    }],
                    "relations": []
                }),
            ),
            (
                "endpoint",
                serde_json::json!({
                    "kind": "endpoint",
                    "method": "get",
                    "path": "/orders",
                    "summary": null,
                    "auth": {"kind": "open", "reason": "public read"},
                    "params": [],
                    "request": null,
                    "responses": [{
                        "status": 200,
                        "description": "ok",
                        "kind": null,
                        "headers": [],
                        "body": {"content_type": null, "example": null}
                    }]
                }),
            ),
            (
                "matrix",
                serde_json::json!({
                    "kind": "matrix",
                    "columns": ["dev", "prod"],
                    "rows": [{
                        "name": "timeout",
                        "cells": [{"column": "prod", "value": "30s", "kind": null}]
                    }]
                }),
            ),
            (
                "compare",
                serde_json::json!({
                    "kind": "compare",
                    "columns": [{"label": "before", "items": [{"text": "inline", "kind": null}]}]
                }),
            ),
            (
                "stack",
                serde_json::json!({
                    "kind": "stack",
                    "frames": [{"marker": null, "symbol": "main", "note": null, "children": []}]
                }),
            ),
        ]
    }

    #[test]
    fn test_every_deferred_kind_parses_from_minimal_valid_json() {
        for (kind, json) in minimal_valid_json_for_each_deferred_kind() {
            let diagram: Diagram = serde_json::from_value(json).unwrap_or_else(|error| {
                panic!("`{kind}` must parse from its minimal JSON: {error}")
            });
            assert_eq!(
                diagram.kind_name(),
                kind,
                "parsed diagram must report its own kind"
            );
        }
    }

    #[test]
    fn test_every_deferred_kind_round_trips_through_a_serialized_json_string() {
        // Lesson from R2b (`Seq`'s `ActorIndex`/`WireSeqDiagram` fix): a
        // `try_from`-carrying type must also serialize back to a shape that
        // re-parses. `Matrix` gets its own dedicated, richer round-trip test
        // above; this one is the cheap general guard for all 5, matching the
        // canonical-JSON invariant `parsed_plan_round_trips_through_canonical_json`
        // enforces at the whole-plan level.
        for (kind, json) in minimal_valid_json_for_each_deferred_kind() {
            let diagram: Diagram = serde_json::from_value(json).unwrap_or_else(|error| {
                panic!("`{kind}` must parse from its minimal JSON: {error}")
            });
            let serialized = serde_json::to_string(&diagram)
                .unwrap_or_else(|error| panic!("`{kind}` diagram must serialize: {error}"));
            let reparsed: Diagram = serde_json::from_str(&serialized).unwrap_or_else(|error| {
                panic!(
                    "re-parsing a serialized `{kind}` diagram must succeed: {error}\n{serialized}"
                )
            });
            assert_eq!(
                diagram, reparsed,
                "`{kind}` diagram must round-trip through canonical JSON"
            );
        }
    }

    #[test]
    fn test_diagram_rejects_an_unrecognized_kind() {
        let json = serde_json::json!({"kind": "not_a_real_kind"});

        serde_json::from_value::<Diagram>(json).expect_err("an unrecognized kind must be rejected");
    }
}
