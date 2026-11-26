//! The deep renderer for a plan candidate.
//!
//! [`render`] projects a [`Candidate`] and a [`RenderContext`] once into a private
//! presentation model and derives both Markdown and standalone HTML from that one
//! projection, so the two formats can never drift. A non-final candidate carries a
//! `DRAFT` watermark; a final candidate with accepted findings carries an
//! `ACCEPTED WITH N UNRESOLVED FINDINGS` banner before the orientation (design §9).
//!
//! The authored plan comes first: brief, behavior, orientation, sections,
//! decisions, delivery, and unresolved facts. Everything derived from typed
//! exploration and review data follows in one appendix — one collapsed
//! disclosure in HTML, one `Appendix` heading in Markdown — and
//! [`Appendix::Omitted`] drops it entirely. The appendix figures are never
//! authored and never omitted when the data exists in a full render: the
//! evidence → claim map (with `sampled`/`limited` markers), impact areas with
//! their blast-radius lanes, the changed-symbol tree, the as-is call graph, the
//! review timeline, and the resolved discussion. Authored `Block::Figure`
//! diagrams remain for flows, sequences, and states.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::LazyLock;

use maud::{DOCTYPE, Markup, PreEscaped, html as markup};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::html::{IncludeBackground, styled_line_to_highlighted_html};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::evidence::{Completeness, Evidence, SemanticObservation};
use crate::ids::EvidenceId;
use crate::model::claim::{ClaimConclusion, UncertaintyDisposition};
use crate::model::content::{
    Block, ChangedSymbol, DecisionOwner, DecisionState, Language, Orientation, Section,
    SymbolChange,
};
use crate::model::delivery::{DeliveryGraph, DeliveryPlan, Mr, Verify};
use crate::model::diagram::Diagram;
use crate::model::discussion::{BlockAnchor, Comment, open_on, resolved};
use crate::model::impact::{AffectedShape, ImpactEntry, LaneCoverage};
use crate::model::review::{
    CompletedReview, ConsumerVerdict, Finding, LaneVerdictKind, ReviewKind, RulingVerdict,
    Severity, TriageOutcome,
};
use crate::primitives::{Location, NonEmptyText};
use crate::render::autocode::autocode_markdown as autocode;
use crate::render::diagram::{diagram_markdown_fallback, diagram_markup};
use crate::render::markdown::{block_html, inline_markup};
use crate::state::{
    Candidate, DecisionDiff, RawExplorationContext, Reopen, ReopenTarget, ReviewRound, Revision,
    RunState,
};

const CSS: &str = include_str!("plan.css");

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    ThemeSet::load_defaults()
        .themes
        .remove("base16-ocean.dark")
        .unwrap_or_default()
});

/// What the renderer needs beyond the candidate.
///
/// The state label, whether the plan is final, the review history, the accepted
/// findings, the comment overlay, and the reopens and revisions the timeline
/// marks.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    pub state_label: &'a str,
    pub is_final: bool,
    pub history: &'a [ReviewRound],
    pub accepted: &'a [Finding],
    pub comments: &'a [Comment],
    pub reopens: &'a [Reopen],
    pub revisions: &'a [Revision],
}

impl<'a> RenderContext<'a> {
    /// The context a run state supplies, under the given watermark label.
    #[must_use]
    pub fn for_state(state: &'a RunState, state_label: &'a str) -> Self {
        Self {
            state_label,
            is_final: matches!(state, RunState::Final { .. }),
            history: state.history(),
            accepted: state.accepted(),
            comments: state.comments(),
            reopens: state.reopens(),
            revisions: state.revisions(),
        }
    }
}

/// The Markdown and standalone HTML renderings of a plan.
#[derive(Debug, Clone)]
pub struct RenderedPlan {
    pub markdown: String,
    pub html: String,
}

/// Whether a render carries the derived appendix after the authored plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appendix {
    Included,
    /// The brief render: the authored plan alone, with its watermark and banner.
    Omitted,
}

/// Render a candidate into Markdown and standalone HTML.
#[must_use]
pub fn render(
    candidate: &Candidate,
    context: &RenderContext<'_>,
    appendix: Appendix,
) -> RenderedPlan {
    let presentation = project(candidate, context, appendix);
    RenderedPlan {
        markdown: to_markdown(&presentation),
        html: to_html(&presentation),
    }
}

// --- The private presentation projection ---------------------------------------

struct UnresolvedFactRow {
    id: String,
    question: String,
    remedy: String,
}

#[derive(Clone, Copy)]
enum ImpactState {
    Affected,
    Unaffected,
    Uncertain,
    NotApplicable,
}

/// One impact subject, carrying the blast-radius lanes whose coverage names it
/// — so a covered lane renders on its subject's row, not as a second listing of
/// the same subjects.
struct ImpactRow {
    subject: String,
    state: ImpactState,
    shape: Option<AffectedShape>,
    rationale: String,
    references: Vec<String>,
    references_are_ids: bool,
    lanes: Vec<&'static str>,
}

/// Lanes set aside for one shared reason, collapsed into a single line.
struct SetAsideGroup {
    reason: String,
    lanes: Vec<&'static str>,
}

struct ImpactView {
    rows: Vec<ImpactRow>,
    set_aside: Vec<SetAsideGroup>,
}

impl ImpactView {
    const fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.set_aside.is_empty()
    }
}

struct RuledOutRow {
    alternative: String,
    reason: String,
}

struct DecisionRow {
    id: String,
    owner: DecisionOwner,
    question: String,
    answer: String,
    consequences: Vec<String>,
    ruled_out: Vec<RuledOutRow>,
}

/// One evidence item backing a claim, with a marker when the evidence set was
/// sampled rather than exhaustive, and when the evidence states a limitation.
struct ClaimEvidenceRef {
    id: String,
    method: &'static str,
    sampled: bool,
    limited: bool,
}

/// One claim and the evidence that backs it — the evidence → claim map (§9).
struct ClaimEvidenceRow {
    claim: String,
    statement: String,
    evidence: Vec<ClaimEvidenceRef>,
    limitation: Option<String>,
}

/// The limitation one evidence item states, listed once however many claims
/// cite it.
struct EvidenceNote {
    id: String,
    method: &'static str,
    detail: String,
}

/// An approved tool the explorer could not use, in place of its evidence.
struct ToolFailureRow {
    proposal: String,
    reason: String,
}

struct EvidenceMapView {
    claims: Vec<ClaimEvidenceRow>,
    notes: Vec<EvidenceNote>,
    tool_failures: Vec<ToolFailureRow>,
}

impl EvidenceMapView {
    const fn is_empty(&self) -> bool {
        self.claims.is_empty() && self.notes.is_empty() && self.tool_failures.is_empty()
    }
}

/// One node in the changed-symbol tree: a directory, a file, or a symbol.
struct SymbolTreeNode {
    label: String,
    /// Absent for directories/files; present with the change kind for symbols.
    change: Option<SymbolChange>,
    depth: usize,
}

/// One as-is call edge, caller → callee, as `path:line` anchors.
struct CallEdgeRow {
    caller: String,
    callee: String,
}

/// One finding in the review timeline, coloured by severity, with the triage or
/// ruling outcome recorded against it.
struct TimelineFinding {
    id: String,
    severity: Severity,
    statement: String,
    outcome: String,
}

/// One lane verdict or consumer check in the timeline.
struct TimelineCheck {
    label: String,
    verdict: String,
    contested: bool,
}

/// One entry in the review timeline: a review wave, or a marker where the
/// candidate was revised or the plan reopened.
enum TimelineEntry {
    Wave(TimelineWave),
    Marker(TimelineMarker),
}

/// A revision or reopen between waves, with its note or reason.
struct TimelineMarker {
    label: String,
    detail: String,
    /// What the marker carried for the user to see: set-aside comments,
    /// changes to user-owned decisions.
    items: Vec<String>,
}

/// One wave in the review timeline.
struct TimelineWave {
    wave: u32,
    reviews: Vec<String>,
    findings: Vec<TimelineFinding>,
    lanes: Vec<TimelineCheck>,
    consumers: Vec<TimelineCheck>,
}

struct RequirementRow {
    id: String,
    statement: String,
    claims: Vec<String>,
}

struct CriterionRow {
    id: String,
    statement: String,
    verify: String,
    owner: Option<String>,
}

struct MrRow {
    id: String,
    title: String,
    needs: Vec<String>,
    satisfies: Vec<String>,
    standalone: String,
}

struct DeliveryView {
    requirements: Vec<RequirementRow>,
    criteria: Vec<CriterionRow>,
    /// The merge requests in dependency order: each stage may land in parallel
    /// once every earlier stage has landed. This one listing is the delivery
    /// graph — no second figure repeats the same merge requests.
    stages: Vec<Vec<MrRow>>,
}

impl DeliveryView {
    const fn has_content(&self) -> bool {
        !self.requirements.is_empty() || !self.criteria.is_empty() || !self.stages.is_empty()
    }
}

struct GlossaryRow {
    term: String,
    definition: String,
}

struct OrientationView {
    placement: String,
    touched_flows: Vec<String>,
    glossary: Vec<GlossaryRow>,
}

enum BlockView {
    Prose {
        text: String,
    },
    Code {
        lang: Language,
        caption: Option<String>,
        code: String,
    },
    Figure {
        diagram: Diagram,
    },
}

struct SectionView {
    heading: String,
    blocks: Vec<BlockView>,
}

/// One accepted finding shown in the acceptance banner.
struct AcceptedRow {
    id: String,
    severity: Severity,
    statement: String,
}

/// The acceptance banner: the accepted findings and their severity tallies.
struct AcceptedBanner {
    findings: Vec<AcceptedRow>,
    blocking: usize,
    major: usize,
}

impl AcceptedBanner {
    const fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }
}

/// The single presentation model both formats render from.
struct PlanPresentation {
    title: Option<String>,
    heading: String,
    scope: String,
    out_of_scope: String,
    current_behavior: String,
    target_behavior: String,
    /// Up to two authored flow/sequence/state figures (a current-flow and a
    /// target-flow) hoisted to dominate the behavior section as the primary
    /// explanation of the change; never derived from the behavior prose.
    /// `behavior_figures_at` are the section/block positions they were taken
    /// from, skipped in the narrative so each renders exactly once.
    behavior_figures: Vec<Diagram>,
    behavior_figures_at: Vec<(usize, usize)>,
    /// The `DRAFT — <state>` watermark, present on a non-final candidate.
    watermark: Option<String>,
    accepted: AcceptedBanner,
    orientation: Option<OrientationView>,
    sections: Vec<SectionView>,
    unresolved_facts: Vec<UnresolvedFactRow>,
    decisions: Vec<DecisionRow>,
    delivery: DeliveryView,
    comments: Vec<Comment>,
    /// Absent in a brief render.
    appendix: Option<AppendixView>,
}

/// Everything derived from exploration and review data, rendered after the
/// authored plan as one appendix.
struct AppendixView {
    evidence_map: EvidenceMapView,
    impact: ImpactView,
    changed_symbols: Vec<SymbolTreeNode>,
    call_graph: Vec<CallEdgeRow>,
    timeline: Vec<TimelineEntry>,
    discussion: Vec<Comment>,
}

impl AppendixView {
    const fn is_empty(&self) -> bool {
        self.evidence_map.is_empty()
            && self.impact.is_empty()
            && self.changed_symbols.is_empty()
            && self.call_graph.is_empty()
            && self.timeline.is_empty()
            && self.discussion.is_empty()
    }
}

fn project(candidate: &Candidate, ctx: &RenderContext<'_>, appendix: Appendix) -> PlanPresentation {
    let raw = candidate.raw();
    let exploration = &raw.exploration;
    let content = &raw.content;

    let sections = sections_view(&content.sections);
    let behavior = find_behavior_figures(&sections, ctx.comments);
    let behavior_figures_at: Vec<(usize, usize)> = behavior
        .iter()
        .map(|(section_index, block_index, _)| (*section_index, *block_index))
        .collect();
    let behavior_figures: Vec<Diagram> = behavior
        .into_iter()
        .map(|(_, _, diagram)| diagram)
        .collect();
    let appendix = match appendix {
        Appendix::Included => Some(AppendixView {
            evidence_map: evidence_map(exploration),
            impact: impact(exploration),
            changed_symbols: changed_symbol_tree(content.changed_symbols.as_slice()),
            call_graph: call_graph(exploration),
            timeline: timeline(ctx.reopens, ctx.history, ctx.revisions),
            discussion: resolved(ctx.comments).into_iter().cloned().collect(),
        }),
        Appendix::Omitted => None,
    };

    PlanPresentation {
        title: raw.brief.title.as_ref().map(|t| t.as_str().to_owned()),
        heading: raw.brief.summary.as_str().to_owned(),
        scope: raw.brief.scope.as_str().to_owned(),
        out_of_scope: raw.brief.out_of_scope.as_str().to_owned(),
        current_behavior: content.current_behavior.as_str().to_owned(),
        target_behavior: content.target_behavior.as_str().to_owned(),
        behavior_figures,
        behavior_figures_at,
        watermark: (!ctx.is_final).then(|| format!("DRAFT — {}", ctx.state_label)),
        accepted: accepted_banner(ctx.accepted),
        orientation: content.orientation.as_ref().map(orientation_view),
        sections,
        unresolved_facts: unresolved_facts(exploration),
        decisions: decisions(&raw.decisions),
        delivery: delivery(&raw.delivery),
        comments: ctx.comments.to_vec(),
        appendix,
    }
}

fn accepted_banner(accepted: &[Finding]) -> AcceptedBanner {
    let blocking = accepted
        .iter()
        .filter(|finding| finding.severity == Severity::Blocking)
        .count();
    let major = accepted
        .iter()
        .filter(|finding| finding.severity == Severity::Major)
        .count();
    AcceptedBanner {
        findings: accepted
            .iter()
            .map(|finding| AcceptedRow {
                id: finding.id.as_str().to_owned(),
                severity: finding.severity,
                statement: finding.statement.as_str().to_owned(),
            })
            .collect(),
        blocking,
        major,
    }
}

fn orientation_view(orientation: &Orientation) -> OrientationView {
    OrientationView {
        placement: orientation.placement.as_str().to_owned(),
        touched_flows: orientation
            .touched_flows
            .iter()
            .map(NonEmptyText::as_str)
            .map(str::to_owned)
            .collect(),
        glossary: orientation
            .glossary
            .iter()
            .map(|term| GlossaryRow {
                term: term.term.as_str().to_owned(),
                definition: term.definition.as_str().to_owned(),
            })
            .collect(),
    }
}

fn sections_view(sections: &[Section]) -> Vec<SectionView> {
    sections.iter().map(section_view).collect()
}

fn section_view(section: &Section) -> SectionView {
    SectionView {
        heading: section.heading.as_str().to_owned(),
        blocks: section.blocks.iter().map(block_view).collect(),
    }
}

fn block_view(block: &Block) -> BlockView {
    match block {
        Block::Prose { text } => BlockView::Prose {
            text: text.as_str().to_owned(),
        },
        Block::Code { excerpt } => BlockView::Code {
            lang: excerpt.lang,
            caption: excerpt
                .caption
                .as_ref()
                .map(NonEmptyText::as_str)
                .map(str::to_owned),
            code: excerpt.code.as_str().to_owned(),
        },
        Block::Figure { diagram } => BlockView::Figure {
            diagram: diagram.clone(),
        },
    }
}

/// Up to the first two authored flow/sequence/state figures from the behavior
/// section (a current-flow and a target-flow), each with the section/block it
/// sits at so the behavior header can lead with them and the narrative can skip
/// those blocks. Only a control-flow-shaped figure — the kinds that explain a
/// behavior change — is eligible; the behavior prose is never turned into a
/// figure.
///
/// The behavior section is the first section that authors such a figure; a
/// control-flow figure in any later section is left in its own section, so an
/// unrelated one (a protocol lifecycle authored elsewhere) is never mislabeled
/// as the behavior nor its section emptied. A figure carrying an open comment is
/// also left in place, so hoisting never strips the comment's inline anchor from
/// both renders.
fn find_behavior_figures(
    sections: &[SectionView],
    comments: &[Comment],
) -> Vec<(usize, usize, Diagram)> {
    let Some((section_index, section)) = sections.iter().enumerate().find(|(_, section)| {
        section.blocks.iter().any(
            |block| matches!(block, BlockView::Figure { diagram } if is_behavior_diagram(diagram)),
        )
    }) else {
        return Vec::new();
    };
    let mut found: Vec<(usize, usize, Diagram)> = Vec::new();
    for (block_index, block) in section.blocks.iter().enumerate() {
        if let BlockView::Figure { diagram } = block
            && is_behavior_diagram(diagram)
        {
            let anchor = BlockAnchor::new(cast_index(section_index), cast_index(block_index));
            if !open_on(comments, anchor).is_empty() {
                continue;
            }
            found.push((section_index, block_index, diagram.clone()));
            if found.len() == 2 {
                break;
            }
        }
    }
    found
}

const fn is_behavior_diagram(diagram: &Diagram) -> bool {
    matches!(
        diagram,
        Diagram::Flow { .. } | Diagram::Seq { .. } | Diagram::States { .. }
    )
}

fn unresolved_facts(exploration: &RawExplorationContext) -> Vec<UnresolvedFactRow> {
    exploration
        .uncertainties
        .iter()
        .map(|fact| UnresolvedFactRow {
            id: fact.id.as_str().to_owned(),
            question: fact.question.as_str().to_owned(),
            remedy: remedy(&fact.disposition),
        })
        .collect()
}

fn remedy(disposition: &UncertaintyDisposition) -> String {
    match disposition {
        UncertaintyDisposition::Open {
            proposed_capability: Some(proposal),
        } => format!("Proposed capability: {proposal}"),
        UncertaintyDisposition::Open {
            proposed_capability: None,
        } => "Open — no capability proposed".to_owned(),
        UncertaintyDisposition::ContainedByCriterion { criterion } => {
            format!("Contained by criterion: {criterion}")
        }
        UncertaintyDisposition::AcceptedByUser { decision } => {
            format!("Accepted by user (decision {decision})")
        }
    }
}

/// Impact subjects with the lanes that cover them, plus the set-aside lanes
/// grouped by their shared reason. Every covered subject is a proven impact
/// subject, so folding coverage onto the rows drops nothing.
fn impact(exploration: &RawExplorationContext) -> ImpactView {
    let mut set_aside: Vec<SetAsideGroup> = Vec::new();
    for entry in &exploration.coverage {
        if let LaneCoverage::NotConsidered { reason } = &entry.coverage {
            match set_aside
                .iter_mut()
                .find(|group| group.reason == reason.as_str())
            {
                Some(group) => group.lanes.push(entry.lane.label()),
                None => set_aside.push(SetAsideGroup {
                    reason: reason.as_str().to_owned(),
                    lanes: vec![entry.lane.label()],
                }),
            }
        }
    }
    ImpactView {
        rows: exploration
            .impact
            .iter()
            .map(|assessment| impact_row(assessment, exploration))
            .collect(),
        set_aside,
    }
}

fn impact_row(
    assessment: &crate::model::impact::ImpactAssessment,
    exploration: &RawExplorationContext,
) -> ImpactRow {
    let (state, shape, rationale, references, references_are_ids) = match &assessment.entry {
        ImpactEntry::Affected {
            rationale,
            shape,
            claims,
        } => (
            ImpactState::Affected,
            Some(*shape),
            rationale.as_str().to_owned(),
            claims
                .iter()
                .map(|claim| claim.as_str().to_owned())
                .collect(),
            true,
        ),
        ImpactEntry::Unaffected { rationale, search } => (
            ImpactState::Unaffected,
            None,
            rationale.as_str().to_owned(),
            vec![search.as_str().to_owned()],
            false,
        ),
        ImpactEntry::Uncertain {
            rationale,
            uncertainty,
        } => (
            ImpactState::Uncertain,
            None,
            rationale.as_str().to_owned(),
            vec![uncertainty.as_str().to_owned()],
            false,
        ),
        ImpactEntry::NotApplicable { rationale } => (
            ImpactState::NotApplicable,
            None,
            rationale.as_str().to_owned(),
            vec![dash()],
            false,
        ),
    };
    let lanes = exploration
        .coverage
        .iter()
        .filter(|entry| match &entry.coverage {
            LaneCoverage::Covered { subjects } => subjects
                .iter()
                .any(|subject| subject == &assessment.subject),
            LaneCoverage::NotConsidered { .. } => false,
        })
        .map(|entry| entry.lane.label())
        .collect();
    ImpactRow {
        subject: assessment.subject.as_str().to_owned(),
        state,
        shape,
        rationale,
        references,
        references_are_ids,
        lanes,
    }
}

/// The evidence → claim map with every stated limitation folded in: a limited
/// claim carries its limitation on its own row, and each evidence item's
/// limitation is listed once under the map, its citations marked `limited`.
fn evidence_map(exploration: &RawExplorationContext) -> EvidenceMapView {
    let by_id: BTreeMap<&EvidenceId, &Evidence> = exploration
        .evidence
        .iter()
        .map(|item| (evidence_id(item), item))
        .collect();
    let claims = exploration
        .claims
        .iter()
        .map(|claim| ClaimEvidenceRow {
            claim: claim.id.as_str().to_owned(),
            statement: claim.statement.as_str().to_owned(),
            evidence: claim
                .evidence
                .iter()
                .map(|reference| {
                    let item = by_id.get(reference);
                    ClaimEvidenceRef {
                        id: reference.as_str().to_owned(),
                        method: item.map_or("?", |item| evidence_method(item)),
                        sampled: item.is_some_and(|item| evidence_sampled(item)),
                        limited: item.is_some_and(|item| evidence_limitation(item).is_some()),
                    }
                })
                .collect(),
            limitation: match &claim.conclusion {
                ClaimConclusion::Limited { limitation } => Some(limitation.as_str().to_owned()),
                ClaimConclusion::Established => None,
            },
        })
        .collect();
    let notes = exploration
        .evidence
        .iter()
        .filter_map(|item| {
            evidence_limitation(item).map(|detail| EvidenceNote {
                id: evidence_id(item).as_str().to_owned(),
                method: evidence_method(item),
                detail: detail.to_owned(),
            })
        })
        .collect();
    let tool_failures = exploration
        .tool_failures
        .iter()
        .map(|failure| ToolFailureRow {
            proposal: failure.proposal.as_str().to_owned(),
            reason: failure.reason.as_str().to_owned(),
        })
        .collect();
    EvidenceMapView {
        claims,
        notes,
        tool_failures,
    }
}

/// Build the changed-symbol tree: directories nested, files under directories,
/// symbols under files (design §9). Flattened to depth-tagged rows for rendering.
fn changed_symbol_tree(symbols: &[ChangedSymbol]) -> Vec<SymbolTreeNode> {
    // Group symbols by file path, then walk the directory prefixes.
    let mut by_file: BTreeMap<String, Vec<&ChangedSymbol>> = BTreeMap::new();
    for symbol in symbols {
        by_file
            .entry(symbol.path.to_string())
            .or_default()
            .push(symbol);
    }
    let mut rows: Vec<SymbolTreeNode> = Vec::new();
    let mut open_dirs: Vec<&str> = Vec::new();
    for (path, file_symbols) in &by_file {
        let components: Vec<&str> = path.split('/').collect();
        let (dirs, file) = match components.split_last() {
            Some((file, dirs)) => (dirs, *file),
            None => (&[][..], path.as_str()),
        };
        // Emit directory rows for any prefix not already open.
        for (depth, dir) in dirs.iter().enumerate() {
            if open_dirs.get(depth) != Some(dir) {
                open_dirs.truncate(depth);
                open_dirs.push(dir);
                rows.push(SymbolTreeNode {
                    label: format!("{dir}/"),
                    change: None,
                    depth,
                });
            }
        }
        open_dirs.truncate(dirs.len());
        let file_depth = dirs.len();
        rows.push(SymbolTreeNode {
            label: file.to_owned(),
            change: None,
            depth: file_depth,
        });
        for symbol in file_symbols {
            rows.push(SymbolTreeNode {
                label: symbol.symbol.as_str().to_owned(),
                change: Some(symbol.change),
                depth: file_depth + 1,
            });
        }
    }
    rows
}

fn call_graph(exploration: &RawExplorationContext) -> Vec<CallEdgeRow> {
    exploration
        .evidence
        .iter()
        .filter_map(|item| match item {
            Evidence::Semantic(semantic) => match &semantic.observation {
                SemanticObservation::CallEdges { edges, .. } => Some(edges),
                _ => None,
            },
            _ => None,
        })
        .flatten()
        .map(|edge| CallEdgeRow {
            caller: location_label(&edge.caller),
            callee: location_label(&edge.callee),
        })
        .collect()
}

fn location_label(location: &Location) -> String {
    format!("{}:{}", location.path, location.line)
}

fn decisions(decisions: &[crate::model::content::Decision]) -> Vec<DecisionRow> {
    decisions
        .iter()
        .map(|decision| {
            let (answer, consequences, ruled_out) = match &decision.state {
                DecisionState::Proposed => {
                    let label = match decision.owner {
                        DecisionOwner::User => "Proposed — awaiting the user",
                        DecisionOwner::Planner => "Proposed — not yet settled",
                    };
                    (label.to_owned(), Vec::new(), Vec::new())
                }
                DecisionState::Answered {
                    answer,
                    consequences,
                    ruled_out,
                } => (
                    answer.as_str().to_owned(),
                    consequences.iter().map(|c| c.as_str().to_owned()).collect(),
                    ruled_out
                        .iter()
                        .map(|entry| RuledOutRow {
                            alternative: entry.alternative.as_str().to_owned(),
                            reason: entry.reason.as_str().to_owned(),
                        })
                        .collect(),
                ),
            };
            DecisionRow {
                id: decision.id.as_str().to_owned(),
                owner: decision.owner,
                question: decision.question.as_str().to_owned(),
                answer,
                consequences,
                ruled_out,
            }
        })
        .collect()
}

/// Build the review timeline (design §9): the rounds of every candidate a reopen
/// set aside, each closed by its reopen marker, then the current candidate's
/// rounds — with a marker before the wave each revision entered.
fn timeline(
    reopens: &[Reopen],
    history: &[ReviewRound],
    revisions: &[Revision],
) -> Vec<TimelineEntry> {
    let mut entries = Vec::new();
    for reopen in reopens {
        candidate_timeline(&reopen.history, &reopen.revisions, &mut entries);
        entries.push(TimelineEntry::Marker(TimelineMarker {
            label: match reopen.target {
                ReopenTarget::Exploration => "Reopened exploration".to_owned(),
                ReopenTarget::Draft => "Reopened draft".to_owned(),
            },
            detail: reopen.reason.as_str().to_owned(),
            items: comment_items(&reopen.comments),
        }));
    }
    candidate_timeline(history, revisions, &mut entries);
    entries
}

fn candidate_timeline(
    history: &[ReviewRound],
    revisions: &[Revision],
    entries: &mut Vec<TimelineEntry>,
) {
    let mut pending = revisions.iter().peekable();
    for wave in waves(history) {
        while let Some(revision) = pending.next_if(|revision| revision.wave.get() <= wave.wave) {
            entries.push(revision_marker(revision));
        }
        entries.push(TimelineEntry::Wave(wave));
    }
    entries.extend(pending.map(revision_marker));
}

fn revision_marker(revision: &Revision) -> TimelineEntry {
    let mut detail = revision.note.as_str().to_owned();
    if !revision.addresses.is_empty() {
        let _ = write!(
            detail,
            " (addresses {})",
            join(revision.addresses.iter().map(crate::ids::FindingId::as_str))
        );
    }
    let items = revision
        .decision_changes
        .iter()
        .map(decision_change_item)
        .chain(comment_items(&revision.comments))
        .collect();
    TimelineEntry::Marker(TimelineMarker {
        label: format!("Revised → wave {}", revision.wave.get()),
        detail,
        items,
    })
}

fn comment_items(comments: &[Comment]) -> Vec<String> {
    comments
        .iter()
        .map(|comment| {
            format!(
                "comment on {}{}: {}",
                comment.anchor.token(),
                if comment.resolved { " (resolved)" } else { "" },
                comment.text
            )
        })
        .collect()
}

fn decision_change_item(change: &DecisionDiff) -> String {
    let list = |items: &[NonEmptyText]| join(items.iter().map(NonEmptyText::as_str));
    let answer = |answer: &Option<NonEmptyText>| {
        answer
            .as_ref()
            .map_or_else(|| "(none)".to_owned(), ToString::to_string)
    };
    match change {
        DecisionDiff::Added { decision, after } => {
            format!("decision `{decision}` added: {}", list(after))
        }
        DecisionDiff::Removed { decision, before } => {
            format!("decision `{decision}` removed: {}", list(before))
        }
        DecisionDiff::Answer {
            decision,
            before,
            after,
        } => format!(
            "decision `{decision}` answer: {} → {}",
            answer(before),
            answer(after)
        ),
        DecisionDiff::Alternatives {
            decision,
            before,
            after,
        } => format!(
            "decision `{decision}` alternatives: {} → {}",
            list(before),
            list(after)
        ),
    }
}

/// One timeline wave per history round, findings coloured by severity with their
/// triage/ruling outcome, lane verdicts, and consumer checks.
fn waves(history: &[ReviewRound]) -> Vec<TimelineWave> {
    history
        .iter()
        .map(|round| {
            let reviews: Vec<String> = round
                .reviews
                .iter()
                .map(|review| review_kind_label(review.kind()))
                .collect();
            let findings: Vec<TimelineFinding> = round
                .reviews
                .iter()
                .flat_map(CompletedReview::findings)
                .map(|finding| TimelineFinding {
                    id: finding.id.as_str().to_owned(),
                    severity: finding.severity,
                    statement: finding.statement.as_str().to_owned(),
                    outcome: finding_outcome(round, &finding.id),
                })
                .collect();
            let (lanes, consumers) = round
                .reviews
                .iter()
                .find_map(|review| match review {
                    CompletedReview::Integrated {
                        lanes, consumers, ..
                    } => Some((lane_checks(lanes), consumer_checks(consumers))),
                    CompletedReview::Specialist { .. } => None,
                })
                .unwrap_or_default();
            TimelineWave {
                wave: round.wave.get(),
                reviews,
                findings,
                lanes,
                consumers,
            }
        })
        .collect()
}

fn lane_checks(lanes: &[crate::model::review::LaneVerdict]) -> Vec<TimelineCheck> {
    lanes
        .iter()
        .map(|verdict| match &verdict.verdict {
            LaneVerdictKind::Confirmed => TimelineCheck {
                label: verdict.lane.label().to_owned(),
                verdict: "confirmed".to_owned(),
                contested: false,
            },
            LaneVerdictKind::Contested { finding } => TimelineCheck {
                label: verdict.lane.label().to_owned(),
                verdict: format!("contested by {finding}"),
                contested: true,
            },
        })
        .collect()
}

fn consumer_checks(consumers: &[crate::model::review::ConsumerCheck]) -> Vec<TimelineCheck> {
    consumers
        .iter()
        .map(|check| match &check.verdict {
            ConsumerVerdict::AllAccounted => TimelineCheck {
                label: check.symbol.as_str().to_owned(),
                verdict: format!("all {} references accounted", check.references_found),
                contested: false,
            },
            ConsumerVerdict::Missing { finding } => TimelineCheck {
                label: check.symbol.as_str().to_owned(),
                verdict: format!("missing consumer ({finding})"),
                contested: true,
            },
        })
        .collect()
}

/// The triage or ruling outcome recorded against one finding in a round.
fn finding_outcome(round: &ReviewRound, finding: &crate::ids::FindingId) -> String {
    if let Some(triage) = round.triage.iter().find(|t| &t.finding == finding) {
        return match &triage.outcome {
            TriageOutcome::Fix { affected } => {
                format!("fixed ({} regions)", affected.as_slice().len())
            }
            TriageOutcome::Reject { rationale } => format!("rejected: {}", rationale.as_str()),
        };
    }
    if let Some(ruling) = round.rulings.iter().find(|r| &r.finding == finding) {
        return match ruling.verdict {
            RulingVerdict::MustFix => "user: must fix".to_owned(),
            RulingVerdict::Dismiss => "user: dismissed".to_owned(),
            RulingVerdict::Accept => "user: accepted".to_owned(),
        };
    }
    "carried".to_owned()
}

fn review_kind_label(kind: ReviewKind) -> String {
    match kind {
        ReviewKind::Integrated => "Integrated".to_owned(),
        ReviewKind::Specialist { focus } => format!("Specialist: {}", focus_name(focus)),
    }
}

fn delivery(plan: &DeliveryPlan) -> DeliveryView {
    DeliveryView {
        requirements: plan
            .requirements
            .iter()
            .map(|requirement| RequirementRow {
                id: requirement.id.as_str().to_owned(),
                statement: requirement.statement.as_str().to_owned(),
                claims: requirement
                    .claims
                    .iter()
                    .map(|c| c.as_str().to_owned())
                    .collect(),
            })
            .collect(),
        criteria: plan
            .acceptance_criteria
            .iter()
            .map(|criterion| CriterionRow {
                id: criterion.id.as_str().to_owned(),
                statement: criterion.statement.as_str().to_owned(),
                verify: verify_label(&criterion.verify),
                owner: plan
                    .mrs
                    .iter()
                    .find(|mr| mr.satisfies.iter().any(|owned| owned == &criterion.id))
                    .map(|mr| mr.id.as_str().to_owned()),
            })
            .collect(),
        stages: mr_stages(plan),
    }
}

fn mr_row(mr: &Mr) -> MrRow {
    MrRow {
        id: mr.id.as_str().to_owned(),
        title: mr.title.as_str().to_owned(),
        needs: mr.needs.iter().map(|id| id.as_str().to_owned()).collect(),
        satisfies: mr
            .satisfies
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        standalone: mr.standalone_behavior.as_str().to_owned(),
    }
}

/// The merge requests grouped into dependency stages. A candidate's delivery
/// graph is proven acyclic, so the one-stage fallback in authored order only
/// guards against ever dropping a merge request from the render.
fn mr_stages(plan: &DeliveryPlan) -> Vec<Vec<MrRow>> {
    let by_id: BTreeMap<&str, &Mr> = plan.mrs.iter().map(|mr| (mr.id.as_str(), mr)).collect();
    let flat = || -> Vec<Vec<MrRow>> {
        if plan.mrs.is_empty() {
            Vec::new()
        } else {
            vec![plan.mrs.iter().map(mr_row).collect()]
        }
    };
    let Ok(graph) = DeliveryGraph::parse(plan) else {
        return flat();
    };
    let stages: Vec<Vec<MrRow>> = graph
        .waves()
        .as_slice()
        .iter()
        .map(|wave| {
            wave.iter()
                .filter_map(|mr_id| by_id.get(mr_id.as_str()))
                .map(|mr| mr_row(mr))
                .collect()
        })
        .collect();
    let staged_count: usize = stages.iter().map(Vec::len).sum();
    if staged_count == plan.mrs.len() {
        stages
    } else {
        flat()
    }
}

fn verify_label(verify: &Verify) -> String {
    match verify {
        Verify::Command(command) => {
            let mut text = command.program.as_str().to_owned();
            for argument in &command.args {
                text.push(' ');
                text.push_str(argument.as_str());
            }
            text
        }
        Verify::Manual { procedure } => format!("manual: {}", procedure.as_str()),
    }
}

// --- Evidence helpers ----------------------------------------------------------

const fn evidence_id(evidence: &Evidence) -> &EvidenceId {
    match evidence {
        Evidence::Runtime(item) => &item.id,
        Evidence::Semantic(item) => &item.id,
        Evidence::Structural(item) => &item.id,
        Evidence::Textual(item) => &item.id,
        Evidence::Documentation(item) => &item.id,
        Evidence::History(item) => &item.id,
        Evidence::Inference(item) => &item.id,
    }
}

const fn evidence_method(evidence: &Evidence) -> &'static str {
    match evidence {
        Evidence::Runtime(_) => "Runtime",
        Evidence::Semantic(_) => "Semantic",
        Evidence::Structural(_) => "Structural",
        Evidence::Textual(_) => "Textual",
        Evidence::Documentation(_) => "Documentation",
        Evidence::History(_) => "History",
        Evidence::Inference(_) => "Inference",
    }
}

/// Whether this evidence recorded a sampled (pruned) result set rather than an
/// exhaustive one — the marker the evidence → claim map shows (design §6.1, §9).
fn evidence_sampled(evidence: &Evidence) -> bool {
    let is_sampled = |completeness: Completeness| completeness.is_sampled();
    match evidence {
        Evidence::Semantic(semantic) => match &semantic.observation {
            SemanticObservation::References { completeness, .. }
            | SemanticObservation::Implementations { completeness, .. }
            | SemanticObservation::CallEdges { completeness, .. } => is_sampled(*completeness),
            SemanticObservation::Definitions { .. } => false,
        },
        Evidence::Structural(structural) => match &structural.observation {
            crate::evidence::StructuralObservation::Symbols { completeness, .. } => {
                is_sampled(*completeness)
            }
            crate::evidence::StructuralObservation::Imports { .. } => false,
        },
        Evidence::Textual(textual) => is_sampled(textual.completeness),
        Evidence::Runtime(_)
        | Evidence::Documentation(_)
        | Evidence::History(_)
        | Evidence::Inference(_) => false,
    }
}

fn evidence_limitation(evidence: &Evidence) -> Option<&str> {
    match evidence {
        Evidence::Runtime(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::Semantic(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::Structural(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::Textual(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::Documentation(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::History(item) => item.limitation.as_ref().map(NonEmptyText::as_str),
        Evidence::Inference(_) => None,
    }
}

const fn focus_name(focus: crate::model::content::SpecialistFocus) -> &'static str {
    use crate::model::content::SpecialistFocus;
    match focus {
        SpecialistFocus::Concurrency => "concurrency",
        SpecialistFocus::Migration => "migration",
        SpecialistFocus::Security => "security",
        SpecialistFocus::ProtocolCompatibility => "protocol_compatibility",
        SpecialistFocus::Performance => "performance",
        SpecialistFocus::PublicApi => "public_api",
    }
}

const fn impact_state_label(state: ImpactState) -> &'static str {
    match state {
        ImpactState::Affected => "affected",
        ImpactState::Unaffected => "unaffected",
        ImpactState::Uncertain => "uncertain",
        ImpactState::NotApplicable => "not applicable",
    }
}

const fn impact_state_class(state: ImpactState) -> &'static str {
    match state {
        ImpactState::Affected => "pill imp-affected",
        ImpactState::Unaffected => "pill imp-unaffected",
        ImpactState::Uncertain => "pill imp-uncertain",
        ImpactState::NotApplicable => "pill imp-na",
    }
}

const fn affected_shape_label(shape: AffectedShape) -> &'static str {
    match shape {
        AffectedShape::Behavioral => "behavioral",
        AffectedShape::Mechanical => "mechanical",
        AffectedShape::Additive => "additive",
        AffectedShape::Scoped => "scoped",
        AffectedShape::InStore => "in-store",
    }
}

const fn affected_shape_class(shape: AffectedShape) -> &'static str {
    match shape {
        AffectedShape::Behavioral => "pill shape-behavioral",
        AffectedShape::Mechanical
        | AffectedShape::Additive
        | AffectedShape::Scoped
        | AffectedShape::InStore => "pill shape-plain",
    }
}

/// The severity pill class for a finding, reusing the change-marker family.
const fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Blocking => "pill mark-del",
        Severity::Major => "pill mark-chg",
        Severity::Advisory => "pill mark-keep",
    }
}

const fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Blocking => "blocking",
        Severity::Major => "major",
        Severity::Advisory => "advisory",
    }
}

const fn change_class(change: SymbolChange) -> &'static str {
    match change {
        SymbolChange::Added => "tre-added",
        SymbolChange::Modified => "tre-modified",
        SymbolChange::Removed => "tre-removed",
    }
}

const fn change_label(change: SymbolChange) -> &'static str {
    match change {
        SymbolChange::Added => "added",
        SymbolChange::Modified => "modified",
        SymbolChange::Removed => "removed",
    }
}

fn impact_state_md(area: &ImpactRow) -> String {
    area.shape.map_or_else(
        || impact_state_label(area.state).to_owned(),
        |shape| {
            format!(
                "{} · {}",
                impact_state_label(area.state),
                affected_shape_label(shape)
            )
        },
    )
}

const DASH: &str = "—";

fn dash() -> String {
    DASH.to_owned()
}

fn join<'a>(values: impl Iterator<Item = &'a str>) -> String {
    values.collect::<Vec<_>>().join(", ")
}

fn join_or_dash<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let joined = join(values);
    if joined.is_empty() { dash() } else { joined }
}

fn id_markup(id: &str) -> Markup {
    markup! { code { (id) } }
}

fn code_join<'a>(values: impl Iterator<Item = &'a str>) -> Markup {
    markup! {
        @for (index, value) in values.enumerate() {
            @if index > 0 { ", " }
            code { (value) }
        }
    }
}

fn code_join_or_dash(ids: &[String]) -> Markup {
    markup! {
        @if ids.is_empty() {
            (dash())
        } @else {
            (code_join(ids.iter().map(String::as_str)))
        }
    }
}

// --- Syntax highlighting ---------------------------------------------------------

const fn language_info(lang: Language) -> (&'static str, &'static str) {
    match lang {
        Language::Rust => ("rs", "rust"),
        Language::Python => ("py", "python"),
        Language::TypeScript => ("ts", "typescript"),
        Language::JavaScript => ("js", "javascript"),
        Language::Go => ("go", "go"),
        Language::Json => ("json", "json"),
        Language::Yaml => ("yaml", "yaml"),
        Language::Toml => ("toml", "toml"),
        Language::Bash => ("sh", "bash"),
        Language::Sql => ("sql", "sql"),
        Language::Text => ("txt", "text"),
    }
}

fn syntax_for(lang: Language) -> &'static SyntaxReference {
    let (extension, _) = language_info(lang);
    SYNTAX_SET
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text())
}

pub(super) fn highlighted_code_html(lang: Language, code: &str) -> String {
    let syntax = syntax_for(lang);
    let mut lines = HighlightLines::new(syntax, &THEME);
    let mut out = String::new();
    for line in LinesWithEndings::from(code) {
        let line_html = lines
            .highlight_line(line, &SYNTAX_SET)
            .ok()
            .and_then(|ranges| {
                styled_line_to_highlighted_html(&ranges, IncludeBackground::No).ok()
            });
        match line_html {
            Some(html) => out.push_str(&html),
            None => out.push_str(&escape_html(line)),
        }
    }
    out
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(character),
        }
    }
    out
}

// --- Markdown rendering --------------------------------------------------------

fn to_markdown(plan: &PlanPresentation) -> String {
    let mut out = String::new();
    if let Some(watermark) = &plan.watermark {
        let _ = writeln!(out, "> **{watermark}**\n");
    }
    md_heading(&mut out, plan);
    md_accepted(&mut out, plan);

    let _ = writeln!(out, "- In scope: {}", autocode(&plan.scope));
    let _ = writeln!(out, "- Out of scope: {}\n", autocode(&plan.out_of_scope));
    // Behaviour is never folded: any authored figure heads the area, the
    // current/target prose is always fully visible below it.
    for figure in &plan.behavior_figures {
        let _ = writeln!(out, "{}\n", diagram_markdown_fallback(figure));
    }
    let _ = writeln!(
        out,
        "- Current behavior: {}",
        autocode(&plan.current_behavior)
    );
    let _ = writeln!(
        out,
        "- Target behavior: {}\n",
        autocode(&plan.target_behavior)
    );

    md_orientation(&mut out, plan);
    md_sections(&mut out, plan);
    md_decisions(&mut out, plan);
    md_delivery(&mut out, plan);
    md_unresolved(&mut out, plan);
    if let Some(appendix) = &plan.appendix {
        md_appendix(&mut out, plan, appendix);
    }
    out
}

fn md_decisions(out: &mut String, plan: &PlanPresentation) {
    if plan.decisions.is_empty() {
        return;
    }
    md_section(out, "Decisions");
    for decision in &plan.decisions {
        let _ = writeln!(out, "- **Decision: {}**", decision.question);
        let _ = writeln!(out, "  {}", decision.answer);
        if !decision.consequences.is_empty() {
            let _ = writeln!(out, "  Consequences: {}", decision.consequences.join("; "));
        }
        let _ = writeln!(
            out,
            "  _{} · {}_",
            decision_owner_label(decision.owner),
            decision.id
        );
        if !decision.ruled_out.is_empty() {
            out.push_str("  - Considered and rejected:\n");
            for entry in &decision.ruled_out {
                let _ = writeln!(out, "    - {}", entry.alternative);
                let _ = writeln!(out, "      {}", entry.reason);
            }
        }
    }
    out.push('\n');
}

fn md_unresolved(out: &mut String, plan: &PlanPresentation) {
    if plan.unresolved_facts.is_empty() {
        return;
    }
    md_section(out, "Unresolved facts and remedies");
    for fact in &plan.unresolved_facts {
        let _ = writeln!(out, "- **Fact: {}**", autocode(&fact.question));
        let _ = writeln!(out, "  {} · remedy: {}", fact.id, fact.remedy);
    }
    out.push('\n');
}

const fn decision_owner_label(owner: DecisionOwner) -> &'static str {
    match owner {
        DecisionOwner::User => "User",
        DecisionOwner::Planner => "Planner",
    }
}

const fn decision_owner_class(owner: DecisionOwner) -> &'static str {
    match owner {
        DecisionOwner::User => "pill dec-user",
        DecisionOwner::Planner => "pill dec-planner",
    }
}

fn md_accepted(out: &mut String, plan: &PlanPresentation) {
    if plan.accepted.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "> **ACCEPTED WITH {} UNRESOLVED FINDINGS ({} blocking, {} major)**\n",
        plan.accepted.findings.len(),
        plan.accepted.blocking,
        plan.accepted.major
    );
    for finding in &plan.accepted.findings {
        let _ = writeln!(
            out,
            "> - **[{}] {}**",
            severity_label(finding.severity),
            autocode(&finding.statement)
        );
        let _ = writeln!(out, ">   {}", finding.id);
    }
    out.push('\n');
}

/// The one `Appendix` heading after the authored plan, each derived figure a
/// subsection under it.
fn md_appendix(out: &mut String, plan: &PlanPresentation, appendix: &AppendixView) {
    if appendix.is_empty() {
        return;
    }
    md_section(out, "Appendix");
    md_evidence_map(out, &appendix.evidence_map);
    md_impact(out, &appendix.impact);
    md_changed_symbols(out, &appendix.changed_symbols);
    md_call_graph(out, &appendix.call_graph);
    md_timeline(out, &appendix.timeline);
    md_discussion(out, plan, &appendix.discussion);
}

fn md_evidence_map(out: &mut String, map: &EvidenceMapView) {
    if map.is_empty() {
        return;
    }
    md_subsection(out, "Evidence map");
    for row in &map.claims {
        let evidence = row
            .evidence
            .iter()
            .map(|item| {
                let mut label = format!("{} ({}", item.id, item.method);
                if item.sampled {
                    label.push_str(", sampled");
                }
                if item.limited {
                    label.push_str(", limited");
                }
                label.push(')');
                label
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "- **Claim: {}**", autocode(&row.statement));
        let _ = writeln!(out, "  {} · {}", row.claim, evidence);
        if let Some(limitation) = &row.limitation {
            let _ = writeln!(out, "  Limited: {}", autocode(limitation));
        }
    }
    if !map.notes.is_empty() {
        out.push_str("\nEvidence limitations:\n");
        for note in &map.notes {
            let _ = writeln!(
                out,
                "- {} ({}): {}",
                note.id,
                note.method,
                autocode(&note.detail)
            );
        }
    }
    if !map.tool_failures.is_empty() {
        out.push_str("\nTool failures:\n");
        for failure in &map.tool_failures {
            let _ = writeln!(
                out,
                "- approved tool {} failed: {}",
                failure.proposal,
                autocode(&failure.reason)
            );
        }
    }
    out.push('\n');
}

fn md_changed_symbols(out: &mut String, symbols: &[SymbolTreeNode]) {
    if symbols.is_empty() {
        return;
    }
    md_subsection(out, "Changed symbols");
    for node in symbols {
        let indent = "  ".repeat(node.depth);
        match node.change {
            Some(change) => {
                let _ = writeln!(out, "{indent}- {} [{}]", node.label, change_label(change));
            }
            None => {
                let _ = writeln!(out, "{indent}- {}", node.label);
            }
        }
    }
    out.push('\n');
}

fn md_call_graph(out: &mut String, edges: &[CallEdgeRow]) {
    if edges.is_empty() {
        return;
    }
    md_subsection(out, "As-is call graph");
    for edge in edges {
        let _ = writeln!(out, "- {} → {}", edge.caller, edge.callee);
    }
    out.push('\n');
}

fn md_timeline(out: &mut String, timeline: &[TimelineEntry]) {
    if timeline.is_empty() {
        return;
    }
    md_subsection(
        out,
        &format!("Review timeline — {}", timeline_digest(timeline)),
    );
    for entry in timeline {
        let wave = match entry {
            TimelineEntry::Marker(marker) => {
                let _ = writeln!(out, "- **{}** — {}", marker.label, autocode(&marker.detail));
                for item in &marker.items {
                    let _ = writeln!(out, "  - {item}");
                }
                continue;
            }
            TimelineEntry::Wave(wave) => wave,
        };
        let clear = wave.findings.is_empty()
            && !wave.lanes.iter().any(|lane| lane.contested)
            && !wave.consumers.iter().any(|consumer| consumer.contested);
        let _ = writeln!(out, "- Wave {} ({})", wave.wave, wave.reviews.join(", "));
        for finding in &wave.findings {
            let _ = writeln!(
                out,
                "  - **[{}] {}** — {} · {}",
                severity_label(finding.severity),
                autocode(&finding.statement),
                finding.outcome,
                finding.id
            );
        }
        for lane in wave.lanes.iter().filter(|lane| lane.contested) {
            let _ = writeln!(out, "  - lane {} — {}", lane.label, lane.verdict);
        }
        for consumer in wave.consumers.iter().filter(|consumer| consumer.contested) {
            let _ = writeln!(
                out,
                "  - consumer {} — {}",
                consumer.label, consumer.verdict
            );
        }
        if clear {
            out.push_str("  - all lanes and consumers clear\n");
        }
    }
    out.push('\n');
}

fn md_impact(out: &mut String, impact: &ImpactView) {
    if impact.is_empty() {
        return;
    }
    md_subsection(out, "Impact areas");
    for area in &impact.rows {
        let lanes = if area.lanes.is_empty() {
            String::new()
        } else {
            format!(" · lanes: {}", area.lanes.join(", "))
        };
        let _ = writeln!(
            out,
            "- **{}** [{}]: {} (refs: {}{lanes})",
            autocode(&area.subject),
            impact_state_md(area),
            autocode(&area.rationale),
            area.references.join(", ")
        );
    }
    for group in &impact.set_aside {
        let _ = writeln!(
            out,
            "- Not considered — {}: {}",
            group.lanes.join(", "),
            autocode(&group.reason)
        );
    }
    out.push('\n');
}

fn md_delivery(out: &mut String, plan: &PlanPresentation) {
    if !plan.delivery.has_content() {
        return;
    }
    md_section(out, "Delivery");
    if !plan.delivery.requirements.is_empty() {
        out.push_str("Requirements:\n");
        for requirement in &plan.delivery.requirements {
            let _ = writeln!(out, "- **Req: {}**", autocode(&requirement.statement));
            let _ = writeln!(
                out,
                "  {} · claims: {}",
                requirement.id,
                join_or_dash(requirement.claims.iter().map(String::as_str))
            );
        }
    }
    if !plan.delivery.criteria.is_empty() {
        out.push_str("Acceptance criteria:\n");
        for criterion in &plan.delivery.criteria {
            let _ = writeln!(out, "- **AC: {}**", autocode(&criterion.statement));
            let owner = criterion
                .owner
                .as_deref()
                .map_or_else(String::new, |owner| format!(" · owner: {owner}"));
            let _ = writeln!(
                out,
                "  {} · verify: {}{owner}",
                criterion.id, criterion.verify
            );
        }
    }
    if !plan.delivery.stages.is_empty() {
        out.push_str("Merge requests:\n");
        let staged = plan.delivery.stages.len() > 1;
        for (index, stage) in plan.delivery.stages.iter().enumerate() {
            if staged {
                let _ = writeln!(out, "- Stage {}{}", index + 1, parallel_note(stage.len()));
            }
            let indent = if staged { "  " } else { "" };
            for mr in stage {
                let _ = writeln!(out, "{indent}- **MR: {}**", autocode(&mr.title));
                let _ = writeln!(out, "{indent}  {}", autocode(&mr.standalone));
                let _ = writeln!(
                    out,
                    "{indent}  {} · needs: {} · satisfies: {}",
                    mr.id,
                    join_or_dash(mr.needs.iter().map(String::as_str)),
                    join_or_dash(mr.satisfies.iter().map(String::as_str))
                );
            }
        }
    }
    out.push('\n');
}

fn parallel_note(count: usize) -> String {
    if count > 1 {
        format!(" · {count} in parallel")
    } else {
        String::new()
    }
}

fn md_discussion(out: &mut String, plan: &PlanPresentation, discussion: &[Comment]) {
    if discussion.is_empty() {
        return;
    }
    md_subsection(out, "Discussion");
    for comment in discussion {
        let _ = writeln!(
            out,
            "- {} — {}",
            discussion_source(plan, comment.anchor),
            comment.text.as_str()
        );
    }
    out.push('\n');
}

fn md_heading(out: &mut String, plan: &PlanPresentation) {
    let heading = page_heading(plan);
    let _ = writeln!(out, "# {heading}\n");
    if heading != plan.heading {
        let _ = writeln!(out, "{}\n", plan.heading);
    }
}

fn md_section(out: &mut String, title: &str) {
    let _ = writeln!(out, "## {title}\n");
}

fn md_subsection(out: &mut String, title: &str) {
    let _ = writeln!(out, "### {title}\n");
}

fn md_orientation(out: &mut String, plan: &PlanPresentation) {
    let Some(orientation) = &plan.orientation else {
        return;
    };
    md_section(out, "Orientation");
    let _ = writeln!(out, "- Placement: {}", autocode(&orientation.placement));
    if !orientation.touched_flows.is_empty() {
        out.push_str("- Touched flows:\n");
        for flow in &orientation.touched_flows {
            let _ = writeln!(out, "  - {}", autocode(flow));
        }
    }
    if !orientation.glossary.is_empty() {
        out.push_str("- Glossary:\n");
        for term in &orientation.glossary {
            let _ = writeln!(out, "  - {}: {}", term.term, autocode(&term.definition));
        }
    }
    out.push('\n');
}

fn md_sections(out: &mut String, plan: &PlanPresentation) {
    for (section_index, section) in plan.sections.iter().enumerate() {
        // Skip the figure hoisted to head the behavior section, and the section
        // itself if that leaves it empty; keep the authored block index for anchors.
        let visible: Vec<(usize, &BlockView)> = section
            .blocks
            .iter()
            .enumerate()
            .filter(|(block_index, _)| {
                !plan
                    .behavior_figures_at
                    .contains(&(section_index, *block_index))
            })
            .collect();
        if visible.is_empty() {
            continue;
        }
        md_section(out, &section.heading);
        for (block_index, block) in visible {
            match block {
                BlockView::Prose { text } => {
                    let _ = writeln!(out, "{text}\n");
                }
                BlockView::Code {
                    lang,
                    caption,
                    code,
                } => {
                    let (_, tag) = language_info(*lang);
                    let _ = writeln!(out, "```{tag}\n{code}\n```");
                    if let Some(caption) = caption {
                        let _ = writeln!(out, "*{caption}*\n");
                    } else {
                        out.push('\n');
                    }
                }
                BlockView::Figure { diagram } => {
                    let _ = writeln!(out, "{}\n", diagram_markdown_fallback(diagram));
                }
            }
            let anchor = BlockAnchor::new(cast_index(section_index), cast_index(block_index));
            for comment in open_on(&plan.comments, anchor) {
                let _ = writeln!(out, "> {}\n", comment.text.as_str());
            }
        }
    }
}

// --- HTML rendering ------------------------------------------------------------

fn short_title(summary: &str) -> String {
    const MAX: usize = 72;
    let chars: Vec<char> = summary.chars().collect();
    if chars.len() <= MAX {
        return summary.to_owned();
    }
    let mut cut = MAX;
    while cut > 0 && !chars.get(cut).is_some_and(|c| c.is_whitespace()) {
        cut -= 1;
    }
    if cut == 0 {
        cut = MAX;
    }
    let head: String = chars.iter().take(cut).collect();
    format!("{}…", head.trim_end())
}

fn page_heading(plan: &PlanPresentation) -> String {
    plan.title
        .clone()
        .unwrap_or_else(|| short_title(&plan.heading))
}

fn to_html(plan: &PlanPresentation) -> String {
    let heading = page_heading(plan);
    let body = markup! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (heading) " — Revy plan" }
                style { (PreEscaped(CSS)) }
            }
            body {
                main {
                    @if let Some(watermark) = &plan.watermark {
                        div class="pill mark-chg" style="display:block;text-align:center;margin:.4rem 0" { (watermark) }
                    }
                    (accepted_banner_markup(plan))
                    header class="page-header" {
                        p class="eyebrow" { "Implementation plan" }
                        h1 { (heading) }
                        @if heading != plan.heading {
                            p class="lead" { (inline_markup(&plan.heading)) }
                        }
                        div class="prose" {
                            p { strong { "In scope. " } (inline_markup(&plan.scope)) }
                            p { strong { "Out of scope. " } (inline_markup(&plan.out_of_scope)) }
                        }
                        // Behaviour is never folded. Any authored figure heads the
                        // area; the current/target prose is always fully visible,
                        // full-width, directly below it as supporting detail.
                        @for figure in &plan.behavior_figures {
                            figure class="dg-figure behavior-figure" {
                                (diagram_markup(figure))
                            }
                        }
                        div class="prose behavior-prose" {
                            p { strong { "Current behavior. " } (inline_markup(&plan.current_behavior)) }
                            p { strong { "Target behavior. " } (inline_markup(&plan.target_behavior)) }
                        }
                    }
                    (orientation_section(plan))
                    (narrative_sections(plan))
                    (decision_section(plan))
                    (delivery_section(plan))
                    (unresolved_section(plan))
                    @if let Some(appendix) = &plan.appendix {
                        (appendix_section(plan, appendix))
                    }
                }
            }
        }
    };
    body.into_string()
}

fn accepted_banner_markup(plan: &PlanPresentation) -> Markup {
    markup! {
        @if !plan.accepted.is_empty() {
            section class="section" {
                h2 class="mark-del" {
                    "ACCEPTED WITH " (plan.accepted.findings.len())
                    " UNRESOLVED FINDINGS (" (plan.accepted.blocking) " blocking, "
                    (plan.accepted.major) " major)"
                }
                ul class="lead-list" {
                    @for finding in &plan.accepted.findings {
                        li {
                            p class="item-lead" {
                                strong {
                                    span class=(severity_class(finding.severity)) { (severity_label(finding.severity)) }
                                    " " (inline_markup(&finding.statement))
                                }
                            }
                            small class="item-cap" { (id_markup(&finding.id)) }
                        }
                    }
                }
            }
        }
    }
}

fn orientation_section(plan: &PlanPresentation) -> Markup {
    markup! {
        @if let Some(orientation) = &plan.orientation {
            (section_wrap("Orientation", markup! {
                p class="prose" { (inline_markup(&orientation.placement)) }
                @if !orientation.touched_flows.is_empty() {
                    h3 { "Touched flows" }
                    ul {
                        @for flow in &orientation.touched_flows {
                            li { (inline_markup(flow)) }
                        }
                    }
                }
                @if !orientation.glossary.is_empty() {
                    h3 { "Glossary" }
                    dl {
                        @for term in &orientation.glossary {
                            dt { (inline_markup(&term.term)) }
                            dd { (inline_markup(&term.definition)) }
                        }
                    }
                }
            }))
        }
    }
}

fn narrative_sections(plan: &PlanPresentation) -> Markup {
    markup! {
        @for (section_index, section) in plan.sections.iter().enumerate() {
            // Keep the authored block index (comment anchors depend on it) but drop
            // the one figure hoisted to head the behavior section, and the section
            // itself if that leaves it empty.
            @let visible: Vec<(usize, &BlockView)> = section
                .blocks
                .iter()
                .enumerate()
                .filter(|(block_index, _)| {
                    !plan.behavior_figures_at.contains(&(section_index, *block_index))
                })
                .collect();
            @if !visible.is_empty() {
                (section_wrap(&section.heading, markup! {
                    @for &(block_index, block) in &visible {
                        @let anchor = BlockAnchor::new(cast_index(section_index), cast_index(block_index));
                        (block_markup(block, anchor, &open_on(&plan.comments, anchor)))
                    }
                }))
            }
        }
    }
}

fn cast_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

fn block_markup(block: &BlockView, anchor: BlockAnchor, open: &[&Comment]) -> Markup {
    let token = anchor.token();
    let body = match block {
        BlockView::Prose { text } => markup! { div data-block=(token) { (block_html(text)) } },
        BlockView::Code {
            lang,
            caption,
            code,
        } => {
            let highlighted = highlighted_code_html(*lang, code);
            markup! {
                figure class="code" data-block=(token) {
                    pre class="code" { code { (PreEscaped(highlighted)) } }
                    @if let Some(caption) = caption {
                        figcaption { (caption) }
                    }
                }
            }
        }
        BlockView::Figure { diagram } => markup! {
            figure class="dg-figure" data-block=(token) {
                (diagram_markup(diagram))
            }
        },
    };
    markup! {
        (body)
        @for comment in open {
            blockquote class="revy-comment" { (comment.text.as_str()) }
        }
    }
}

/// One derived figure inside the appendix.
struct AppendixPart {
    title: &'static str,
    body: Markup,
}

/// The single collapsed disclosure after the authored plan, holding every
/// derived figure; its summary names the parts inside.
fn appendix_section(plan: &PlanPresentation, appendix: &AppendixView) -> Markup {
    let parts: Vec<AppendixPart> = [
        evidence_map_part(&appendix.evidence_map),
        impact_part(&appendix.impact),
        changed_symbol_part(&appendix.changed_symbols),
        call_graph_part(&appendix.call_graph),
        timeline_part(&appendix.timeline),
        discussion_part(plan, &appendix.discussion),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        return PreEscaped(String::new());
    }
    let digest = parts
        .iter()
        .map(|part| part.title)
        .collect::<Vec<_>>()
        .join(" · ");
    markup! {
        details class="section secondary appendix" {
            summary {
                h2 { "Appendix" }
                span class="appendix-digest" { (digest) }
            }
            div class="body" {
                @for part in &parts {
                    section class="appendix-part" {
                        h3 { (part.title) }
                        (part.body)
                    }
                }
            }
        }
    }
}

fn evidence_map_part(map: &EvidenceMapView) -> Option<AppendixPart> {
    (!map.is_empty()).then(|| AppendixPart {
        title: "Evidence map",
        body: markup! {
            @if !map.claims.is_empty() {
                table {
                    colgroup { col style="width:62%"; col style="width:38%"; }
                    thead { tr { th { "Claim" } th { "Evidence" } } }
                    tbody {
                        @for row in &map.claims {
                            tr {
                                td {
                                    div class="item-lead" { strong { (inline_markup(&row.statement)) } }
                                    small class="item-cap" { (id_markup(&row.claim)) }
                                    @if let Some(limitation) = &row.limitation {
                                        small class="item-cap" {
                                            span class="pill mark-chg" { "limited" }
                                            " " (inline_markup(limitation))
                                        }
                                    }
                                }
                                td {
                                    @for (index, item) in row.evidence.iter().enumerate() {
                                        @if index > 0 { " " }
                                        span class="pill mark-keep" {
                                            code { (item.id) } " · " (item.method)
                                            @if item.sampled {
                                                " · " span class="pill mark-chg" { "sampled" }
                                            }
                                            @if item.limited {
                                                " · " span class="pill mark-chg" { "limited" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            @if !map.notes.is_empty() {
                p { strong { "Evidence limitations" } }
                ul {
                    @for note in &map.notes {
                        li { (id_markup(&note.id)) " (" (note.method) "): " (inline_markup(&note.detail)) }
                    }
                }
            }
            @if !map.tool_failures.is_empty() {
                p { strong { "Tool failures" } }
                ul {
                    @for failure in &map.tool_failures {
                        li { "approved tool " (id_markup(&failure.proposal)) " failed: " (inline_markup(&failure.reason)) }
                    }
                }
            }
        },
    })
}

fn changed_symbol_part(symbols: &[SymbolTreeNode]) -> Option<AppendixPart> {
    (!symbols.is_empty()).then(|| AppendixPart {
        title: "Changed symbols",
        body: markup! {
            div class="d tree" {
                @for node in symbols {
                    div class="tre-row" style=(format!("padding-left:{}rem", node.depth)) {
                        @match node.change {
                            Some(change) => {
                                span class=(format!("tre-n {}", change_class(change))) { (node.label) }
                                span class=(format!("tre-b {}", change_class(change))) { (change_label(change)) }
                            }
                            None => {
                                span class="tre-n" { (node.label) }
                            }
                        }
                    }
                }
            }
        },
    })
}

fn call_graph_part(edges: &[CallEdgeRow]) -> Option<AppendixPart> {
    (!edges.is_empty()).then(|| AppendixPart {
        title: "As-is call graph",
        body: markup! {
            ul class="dg-crossings" {
                @for edge in edges {
                    li { code { (edge.caller) } " → " code { (edge.callee) } }
                }
            }
        },
    })
}

/// A short outcome token for the timeline digest: `fixed (2 regions)` → `fixed`,
/// `user: must fix` → `must fix`, `rejected: …` → `rejected`.
fn short_outcome(outcome: &str) -> String {
    let trimmed = outcome.strip_prefix("user: ").unwrap_or(outcome);
    let head = trimmed.split(" (").next().unwrap_or(trimmed);
    head.split(':').next().unwrap_or(head).trim().to_owned()
}

/// The one-line digest folded into the timeline `<summary>`: wave count, findings
/// by severity (majors/blocking carrying their shared outcome), and the count of
/// lanes and consumers that came back clear — or the exceptions when they did not.
fn timeline_digest(entries: &[TimelineEntry]) -> String {
    let waves: Vec<&TimelineWave> = entries
        .iter()
        .filter_map(|entry| match entry {
            TimelineEntry::Wave(wave) => Some(wave),
            TimelineEntry::Marker(_) => None,
        })
        .collect();
    let markers = entries.len() - waves.len();
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "{} {}",
        waves.len(),
        if waves.len() == 1 { "wave" } else { "waves" }
    ));
    if markers > 0 {
        parts.push(format!(
            "{markers} {}",
            if markers == 1 {
                "revision/reopen"
            } else {
                "revisions/reopens"
            }
        ));
    }

    for severity in [Severity::Blocking, Severity::Major, Severity::Advisory] {
        let matching: Vec<&TimelineFinding> = waves
            .iter()
            .flat_map(|wave| &wave.findings)
            .filter(|finding| finding.severity == severity)
            .collect();
        let Some(first) = matching.first() else {
            continue;
        };
        let label = severity_label(severity);
        let suffix = if matches!(severity, Severity::Blocking | Severity::Major) {
            let outcome = short_outcome(&first.outcome);
            let uniform = matching
                .iter()
                .all(|finding| short_outcome(&finding.outcome) == outcome);
            if uniform && !outcome.is_empty() {
                format!(" ({outcome})")
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        parts.push(format!("{} {label}{suffix}", matching.len()));
    }

    let mut lanes: BTreeSet<&str> = BTreeSet::new();
    let mut consumers: BTreeSet<&str> = BTreeSet::new();
    let mut contested_lanes = 0usize;
    let mut missing_consumers = 0usize;
    for wave in &waves {
        for lane in &wave.lanes {
            lanes.insert(lane.label.as_str());
            if lane.contested {
                contested_lanes += 1;
            }
        }
        for consumer in &wave.consumers {
            consumers.insert(consumer.label.as_str());
            if consumer.contested {
                missing_consumers += 1;
            }
        }
    }
    if !lanes.is_empty() || !consumers.is_empty() {
        if contested_lanes == 0 && missing_consumers == 0 {
            parts.push(format!(
                "{} lanes + {} consumers all clear",
                lanes.len(),
                consumers.len()
            ));
        } else {
            let mut exceptions: Vec<String> = Vec::new();
            if contested_lanes > 0 {
                exceptions.push(format!("{contested_lanes} contested"));
            }
            if missing_consumers > 0 {
                exceptions.push(format!("{missing_consumers} missing"));
            }
            parts.push(exceptions.join(", "));
        }
    }
    parts.join(" · ")
}

/// The retrospective review timeline (design §9). The digest carries the
/// all-clear lane/consumer counts; the body spells out only the findings and
/// the exceptions (a contested lane, a missing consumer).
fn timeline_part(timeline: &[TimelineEntry]) -> Option<AppendixPart> {
    (!timeline.is_empty()).then(|| AppendixPart {
        title: "Review timeline",
        body: markup! {
            p class="tl-digest" { (timeline_digest(timeline)) }
            div class="timeline" {
                @for entry in timeline {
                    @match entry {
                    TimelineEntry::Marker(marker) => {
                        div class="tl-step tl-marker" {
                            span class="tl-dot" {}
                            div class="tl-body" {
                                div class="tl-n" { (marker.label) }
                                div class="tl-w" { (inline_markup(&marker.detail)) }
                                @for item in &marker.items {
                                    div class="tl-rb" { (inline_markup(item)) }
                                }
                            }
                        }
                    }
                    TimelineEntry::Wave(wave) => {
                    @let clear = wave.findings.is_empty()
                        && !wave.lanes.iter().any(|lane| lane.contested)
                        && !wave.consumers.iter().any(|consumer| consumer.contested);
                    div class="tl-step" {
                        span class="tl-dot" {}
                        div class="tl-body" {
                            div class="tl-n" { "Wave " (wave.wave) }
                            div class="tl-w" { (wave.reviews.join(", ")) }
                            @for finding in &wave.findings {
                                div class="tl-rb" {
                                    p class="item-lead" {
                                        strong {
                                            span class=(severity_class(finding.severity)) { (severity_label(finding.severity)) }
                                            " " (inline_markup(&finding.statement))
                                        }
                                    }
                                    small class="item-cap" { (id_markup(&finding.id)) " · " (finding.outcome) }
                                }
                            }
                            @for lane in wave.lanes.iter().filter(|lane| lane.contested) {
                                div class="tl-rb" {
                                    span class="pill mark-del" { "lane " (lane.label) }
                                    " " (lane.verdict)
                                }
                            }
                            @for consumer in wave.consumers.iter().filter(|consumer| consumer.contested) {
                                div class="tl-rb" {
                                    span class="pill mark-del" { "consumer " (consumer.label) }
                                    " " (consumer.verdict)
                                }
                            }
                            @if clear {
                                div class="tl-clear" { "All lanes and consumers clear." }
                            }
                        }
                    }
                    }
                    }
                }
            }
        },
    })
}

fn section_wrap(heading: &str, body: Markup) -> Markup {
    // Consume `body` up front (it is owned, not borrowed) so the parameter earns
    // its by-value signature; re-wrapping the already-escaped string renders
    // identically to splicing the `Markup` directly.
    let body = PreEscaped(body.into_string());
    markup! {
        section class="section" {
            h2 { (heading) }
            (body)
        }
    }
}

fn section_when_nonempty<T>(heading: &str, items: &[T], body: Markup) -> Markup {
    if items.is_empty() {
        PreEscaped(String::new())
    } else {
        section_wrap(heading, body)
    }
}

fn discussion_part(plan: &PlanPresentation, discussion: &[Comment]) -> Option<AppendixPart> {
    (!discussion.is_empty()).then(|| AppendixPart {
        title: "Discussion",
        body: markup! {
            p { "Resolved review comments." }
            @for comment in discussion {
                div class="disc" {
                    div class="disc-src" { (discussion_source(plan, comment.anchor)) }
                    blockquote class="revy-comment" { (comment.text.as_str()) }
                }
            }
        },
    })
}

fn unresolved_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Unresolved facts and remedies",
        &plan.unresolved_facts,
        markup! {
            ul class="lead-list" {
                @for fact in &plan.unresolved_facts {
                    li {
                        p class="item-lead" { strong { "Fact: " (inline_markup(&fact.question)) } }
                        small class="item-cap" { (id_markup(&fact.id)) " · remedy: " (inline_markup(&fact.remedy)) }
                    }
                }
            }
        },
    )
}

fn impact_part(impact: &ImpactView) -> Option<AppendixPart> {
    (!impact.is_empty()).then(|| AppendixPart {
        title: "Impact areas",
        body: markup! {
            @if !impact.rows.is_empty() {
                table {
                    thead { tr { th { "Area" } th { "State" } th { "Rationale" } th { "References" } th { "Lanes" } } }
                    tbody {
                        @for area in &impact.rows {
                            tr {
                                td { strong { (inline_markup(&area.subject)) } }
                                td {
                                    span class=(impact_state_class(area.state)) {
                                        (impact_state_label(area.state))
                                    }
                                    @if let Some(shape) = area.shape {
                                        " "
                                        span class=(affected_shape_class(shape)) {
                                            (affected_shape_label(shape))
                                        }
                                    }
                                }
                                td { (inline_markup(&area.rationale)) }
                                td {
                                    @if area.references_are_ids {
                                        (code_join(area.references.iter().map(String::as_str)))
                                    } @else {
                                        (area.references.join(", "))
                                    }
                                }
                                td {
                                    @if area.lanes.is_empty() {
                                        (dash())
                                    } @else {
                                        @for lane in &area.lanes {
                                            span class="pill lane-covered" { (lane) } " "
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            @if !impact.set_aside.is_empty() {
                ul class="set-aside" {
                    @for group in &impact.set_aside {
                        li {
                            "Not considered — "
                            @for lane in &group.lanes {
                                span class="pill imp-na" { (lane) } " "
                            }
                            span class="set-aside-why" { (inline_markup(&group.reason)) }
                        }
                    }
                }
            }
        },
    })
}

fn decision_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Decisions",
        &plan.decisions,
        markup! {
            ul class="lead-list decisions" {
                @for decision in &plan.decisions {
                    li {
                        p class="item-lead" { strong { "Decision: " (inline_markup(&decision.question)) } }
                        p class="item-body" { (inline_markup(&decision.answer)) }
                        @if !decision.consequences.is_empty() {
                            ul class="consequences" {
                                @for consequence in &decision.consequences {
                                    li { (inline_markup(consequence)) }
                                }
                            }
                        }
                        small class="item-cap" {
                            span class=(decision_owner_class(decision.owner)) { (decision_owner_label(decision.owner)) }
                            " " (id_markup(&decision.id))
                        }
                        @if !decision.ruled_out.is_empty() {
                            div class="ruled-out" {
                                span class="ruled-out-label" { "Considered and rejected" }
                                ul {
                                    @for entry in &decision.ruled_out {
                                        li class="ro-item" {
                                            span class="ro-alt" { (inline_markup(&entry.alternative)) }
                                            span class="ro-why" { (inline_markup(&entry.reason)) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

fn delivery_section(plan: &PlanPresentation) -> Markup {
    if !plan.delivery.has_content() {
        return PreEscaped(String::new());
    }
    section_wrap(
        "Delivery",
        markup! {
            @if !plan.delivery.requirements.is_empty() {
                h3 { "Requirements" }
                ul class="lead-list" {
                    @for requirement in &plan.delivery.requirements {
                        li {
                            p class="item-lead" { strong { "Req: " (inline_markup(&requirement.statement)) } }
                            small class="item-cap" {
                                (id_markup(&requirement.id)) " · claims: "
                                (code_join_or_dash(&requirement.claims))
                            }
                        }
                    }
                }
            }
            @if !plan.delivery.criteria.is_empty() {
                h3 { "Acceptance criteria" }
                ul class="lead-list" {
                    @for criterion in &plan.delivery.criteria {
                        li {
                            p class="item-lead" { strong { "AC: " (inline_markup(&criterion.statement)) } }
                            small class="item-cap" {
                                (id_markup(&criterion.id)) " · verify: " code { (criterion.verify) }
                                @if let Some(owner) = &criterion.owner {
                                    " · owner: " (id_markup(owner))
                                }
                            }
                        }
                    }
                }
            }
            @if !plan.delivery.stages.is_empty() {
                h3 { "Merge requests" }
                @let staged = plan.delivery.stages.len() > 1;
                @for (index, stage) in plan.delivery.stages.iter().enumerate() {
                    @if staged {
                        p class="mr-stage" { "Stage " (index + 1) (parallel_note(stage.len())) }
                    }
                    ul class="lead-list" {
                        @for mr in stage {
                            li id=(format!("mr-{}", mr.id)) {
                                p class="item-lead" { strong { "MR: " (inline_markup(&mr.title)) } }
                                p class="item-body" { (inline_markup(&mr.standalone)) }
                                small class="item-cap" {
                                    (id_markup(&mr.id)) " · needs: " (code_join_or_dash(&mr.needs))
                                    " · satisfies: " (code_join_or_dash(&mr.satisfies))
                                }
                            }
                        }
                    }
                }
            }
        },
    )
}

fn discussion_source(plan: &PlanPresentation, anchor: BlockAnchor) -> String {
    usize::try_from(anchor.section)
        .ok()
        .and_then(|index| plan.sections.get(index))
        .map_or_else(|| anchor.token(), |section| section.heading.clone())
}

#[cfg(test)]
mod tests {
    use super::short_title;

    #[test]
    fn short_title_keeps_a_headline_length_summary_verbatim() {
        let summary = "Add BLPOP and the three remaining list commands";
        assert_eq!(short_title(summary), summary);
    }

    #[test]
    fn short_title_truncates_a_paragraph_summary_at_a_word_boundary_with_an_ellipsis() {
        let summary = "Add the four remaining Redis list commands to a codebase that already \
                       serves RPUSH and LRANGE through a fixed shape: one file per command under \
                       src/commands implementing the RedisCommand trait, and much more besides.";
        let title = short_title(summary);
        assert!(title.chars().count() <= 73, "{title:?}");
        assert!(title.ends_with('…'), "{title:?}");
    }
}
