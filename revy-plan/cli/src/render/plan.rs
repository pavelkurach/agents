//! The deep renderer for a plan candidate.
//!
//! [`render`] projects a [`Candidate`] and a [`RenderContext`] once into a private
//! presentation model and derives both Markdown and standalone HTML from that one
//! projection, so the two formats can never drift. A non-final candidate carries a
//! `DRAFT` watermark; a final candidate with accepted findings carries an
//! `ACCEPTED WITH N UNRESOLVED FINDINGS` banner before the orientation (design §9).
//!
//! Four figures are derived from typed data and are mandatory renderer output —
//! never authored, never omitted when the data exists: the evidence → claim map
//! (with a `sampled` marker), the changed-symbol tree, the as-is call graph, and
//! the review timeline. Authored `Block::Figure` diagrams remain for flows,
//! sequences, and states.

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
use crate::state::{Candidate, RawExplorationContext, ReviewRound};

const CSS: &str = include_str!("plan.css");

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(|| {
    ThemeSet::load_defaults()
        .themes
        .remove("base16-ocean.dark")
        .unwrap_or_default()
});

/// What the renderer needs beyond the candidate: the state label, whether the plan
/// is final, the review history, the accepted findings, and the comment overlay.
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    pub state_label: &'a str,
    pub is_final: bool,
    pub history: &'a [ReviewRound],
    pub accepted: &'a [Finding],
    pub comments: &'a [Comment],
}

/// The Markdown and standalone HTML renderings of a plan.
#[derive(Debug, Clone)]
pub struct RenderedPlan {
    pub markdown: String,
    pub html: String,
}

/// Render a candidate into Markdown and standalone HTML.
#[must_use]
pub fn render(candidate: &Candidate, context: &RenderContext<'_>) -> RenderedPlan {
    let presentation = project(candidate, context);
    RenderedPlan {
        markdown: to_markdown(&presentation),
        html: to_html(&presentation),
    }
}

// --- The private presentation projection ---------------------------------------

struct Limitation {
    source: String,
    detail: String,
}

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

struct ImpactRow {
    subject: String,
    state: ImpactState,
    shape: Option<AffectedShape>,
    rationale: String,
    references: Vec<String>,
    references_are_ids: bool,
}

struct CoverageRow {
    lane: &'static str,
    covered: bool,
    detail: String,
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
/// sampled rather than exhaustive.
struct ClaimEvidenceRef {
    id: String,
    method: &'static str,
    sampled: bool,
}

/// One claim and the evidence that backs it — the evidence → claim map (§9).
struct ClaimEvidenceRow {
    claim: String,
    statement: String,
    evidence: Vec<ClaimEvidenceRef>,
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

struct MrDagNode {
    id: String,
    title: String,
}

struct MrDagEdge {
    from: String,
    to: String,
}

struct MrDagView {
    waves: Vec<Vec<MrDagNode>>,
    edges: Vec<MrDagEdge>,
}

struct DeliveryView {
    requirements: Vec<RequirementRow>,
    criteria: Vec<CriterionRow>,
    mrs: Vec<MrRow>,
    mr_dag: MrDagView,
}

impl DeliveryView {
    const fn has_content(&self) -> bool {
        !self.requirements.is_empty() || !self.criteria.is_empty() || !self.mrs.is_empty()
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
    evidence_limitations: Vec<Limitation>,
    unresolved_facts: Vec<UnresolvedFactRow>,
    impact_areas: Vec<ImpactRow>,
    coverage: Vec<CoverageRow>,
    evidence_claim_map: Vec<ClaimEvidenceRow>,
    changed_symbols: Vec<SymbolTreeNode>,
    call_graph: Vec<CallEdgeRow>,
    decisions: Vec<DecisionRow>,
    timeline: Vec<TimelineWave>,
    delivery: DeliveryView,
    authored_delivery_dag: bool,
    comments: Vec<Comment>,
}

fn project(candidate: &Candidate, ctx: &RenderContext<'_>) -> PlanPresentation {
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
    let authored_delivery_dag = sections
        .iter()
        .flat_map(|section| &section.blocks)
        .any(|block| {
            matches!(
                block,
                BlockView::Figure {
                    diagram: Diagram::Dag { .. }
                }
            )
        });

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
        evidence_limitations: evidence_limitations(exploration),
        unresolved_facts: unresolved_facts(exploration),
        impact_areas: impact_areas(exploration),
        coverage: coverage(exploration),
        evidence_claim_map: evidence_claim_map(exploration),
        changed_symbols: changed_symbol_tree(content.changed_symbols.as_slice()),
        call_graph: call_graph(exploration),
        decisions: decisions(&raw.decisions),
        timeline: timeline(ctx.history),
        delivery: delivery(&raw.delivery),
        authored_delivery_dag,
        comments: ctx.comments.to_vec(),
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

fn evidence_limitations(exploration: &RawExplorationContext) -> Vec<Limitation> {
    let mut limitations: Vec<Limitation> = Vec::new();
    for item in &exploration.evidence {
        if let Some(detail) = evidence_limitation(item) {
            limitations.push(Limitation {
                source: format!("Evidence {} ({})", evidence_id(item), evidence_method(item)),
                detail: detail.to_owned(),
            });
        }
    }
    for claim in &exploration.claims {
        if let ClaimConclusion::Limited { limitation } = &claim.conclusion {
            limitations.push(Limitation {
                source: format!("Claim {}", claim.id),
                detail: limitation.as_str().to_owned(),
            });
        }
    }
    limitations
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

fn impact_areas(exploration: &RawExplorationContext) -> Vec<ImpactRow> {
    exploration
        .impact
        .iter()
        .map(|assessment| {
            let (state, shape, rationale, references, references_are_ids) = match &assessment.entry
            {
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
            ImpactRow {
                subject: assessment.subject.as_str().to_owned(),
                state,
                shape,
                rationale,
                references,
                references_are_ids,
            }
        })
        .collect()
}

fn coverage(exploration: &RawExplorationContext) -> Vec<CoverageRow> {
    exploration
        .coverage
        .iter()
        .map(|entry| match &entry.coverage {
            LaneCoverage::Covered { subjects } => CoverageRow {
                lane: entry.lane.label(),
                covered: true,
                detail: join(subjects.iter().map(NonEmptyText::as_str)),
            },
            LaneCoverage::NotConsidered { reason } => CoverageRow {
                lane: entry.lane.label(),
                covered: false,
                detail: reason.as_str().to_owned(),
            },
        })
        .collect()
}

fn evidence_claim_map(exploration: &RawExplorationContext) -> Vec<ClaimEvidenceRow> {
    let by_id: BTreeMap<&EvidenceId, &Evidence> = exploration
        .evidence
        .iter()
        .map(|item| (evidence_id(item), item))
        .collect();
    exploration
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
                    }
                })
                .collect(),
        })
        .collect()
}

/// Build the changed-symbol tree: directories nested, files under directories,
/// symbols under files (design §9). Flattened to depth-tagged rows for rendering.
fn changed_symbol_tree(symbols: &[ChangedSymbol]) -> Vec<SymbolTreeNode> {
    // Group symbols by file path, then walk the directory prefixes.
    let mut by_file: BTreeMap<&str, Vec<&ChangedSymbol>> = BTreeMap::new();
    for symbol in symbols {
        by_file
            .entry(symbol.path.as_str())
            .or_default()
            .push(symbol);
    }
    let mut rows: Vec<SymbolTreeNode> = Vec::new();
    let mut open_dirs: Vec<&str> = Vec::new();
    for (path, file_symbols) in &by_file {
        let components: Vec<&str> = path.split('/').collect();
        let (dirs, file) = match components.split_last() {
            Some((file, dirs)) => (dirs, *file),
            None => (&[][..], *path),
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
    format!("{}:{}", location.path.as_str(), location.line)
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

/// Build the review timeline: one wave per history round, findings coloured by
/// severity with their triage/ruling outcome, lane verdicts, and consumer checks
/// (design §9).
fn timeline(history: &[ReviewRound]) -> Vec<TimelineWave> {
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
        mrs: plan
            .mrs
            .iter()
            .map(|mr| MrRow {
                id: mr.id.as_str().to_owned(),
                title: mr.title.as_str().to_owned(),
                needs: mr.needs.iter().map(|id| id.as_str().to_owned()).collect(),
                satisfies: mr
                    .satisfies
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
                standalone: mr.standalone_behavior.as_str().to_owned(),
            })
            .collect(),
        mr_dag: mr_dag_view(plan),
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

fn mr_dag_view(plan: &DeliveryPlan) -> MrDagView {
    let by_id: BTreeMap<&str, &Mr> = plan.mrs.iter().map(|mr| (mr.id.as_str(), mr)).collect();
    let waves: Vec<Vec<MrDagNode>> = DeliveryGraph::parse(plan)
        .map(|graph| {
            graph
                .waves()
                .as_slice()
                .iter()
                .map(|wave| {
                    wave.iter()
                        .filter_map(|mr_id| by_id.get(mr_id.as_str()))
                        .map(|mr| MrDagNode {
                            id: mr.id.as_str().to_owned(),
                            title: mr.title.as_str().to_owned(),
                        })
                        .collect()
                })
                .collect()
        })
        .unwrap_or_default();
    let edges: Vec<MrDagEdge> = plan
        .mrs
        .iter()
        .flat_map(|mr| {
            mr.needs.iter().map(move |need| MrDagEdge {
                from: need.as_str().to_owned(),
                to: mr.id.as_str().to_owned(),
            })
        })
        .collect();
    MrDagView { waves, edges }
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

    if !plan.evidence_limitations.is_empty() {
        md_section(&mut out, "Evidence limitations");
        for limitation in &plan.evidence_limitations {
            let _ = writeln!(
                out,
                "- {}: {}",
                limitation.source,
                autocode(&limitation.detail)
            );
        }
        out.push('\n');
    }

    md_evidence_map(&mut out, plan);
    md_changed_symbols(&mut out, plan);
    md_call_graph(&mut out, plan);

    if !plan.unresolved_facts.is_empty() {
        md_section(&mut out, "Unresolved facts and remedies");
        for fact in &plan.unresolved_facts {
            let _ = writeln!(out, "- **Fact: {}**", autocode(&fact.question));
            let _ = writeln!(out, "  {} · remedy: {}", fact.id, fact.remedy);
        }
        out.push('\n');
    }

    md_impact(&mut out, plan);

    if !plan.decisions.is_empty() {
        md_section(&mut out, "Decisions");
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

    md_delivery(&mut out, plan);
    md_timeline(&mut out, plan);
    md_discussion(&mut out, plan);
    out
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

fn md_evidence_map(out: &mut String, plan: &PlanPresentation) {
    if plan.evidence_claim_map.is_empty() {
        return;
    }
    md_section(out, "Evidence map");
    for row in &plan.evidence_claim_map {
        let evidence = row
            .evidence
            .iter()
            .map(|item| {
                if item.sampled {
                    format!("{} ({}, sampled)", item.id, item.method)
                } else {
                    format!("{} ({})", item.id, item.method)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "- **Claim: {}**", autocode(&row.statement));
        let _ = writeln!(out, "  {} · {}", row.claim, evidence);
    }
    out.push('\n');
}

fn md_changed_symbols(out: &mut String, plan: &PlanPresentation) {
    if plan.changed_symbols.is_empty() {
        return;
    }
    md_section(out, "Changed symbols");
    for node in &plan.changed_symbols {
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

fn md_call_graph(out: &mut String, plan: &PlanPresentation) {
    if plan.call_graph.is_empty() {
        return;
    }
    md_section(out, "As-is call graph");
    for edge in &plan.call_graph {
        let _ = writeln!(out, "- {} → {}", edge.caller, edge.callee);
    }
    out.push('\n');
}

fn md_timeline(out: &mut String, plan: &PlanPresentation) {
    if plan.timeline.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "<details><summary>Review timeline — {}</summary>\n",
        timeline_digest(&plan.timeline)
    );
    for wave in &plan.timeline {
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
    out.push_str("\n</details>\n\n");
}

fn md_impact(out: &mut String, plan: &PlanPresentation) {
    if plan.impact_areas.is_empty() && plan.coverage.is_empty() {
        return;
    }
    md_section(out, "Impact areas");
    for area in &plan.impact_areas {
        let _ = writeln!(
            out,
            "- **{}** [{}]: {} (refs: {})",
            autocode(&area.subject),
            impact_state_md(area),
            autocode(&area.rationale),
            area.references.join(", ")
        );
    }
    if !plan.coverage.is_empty() {
        out.push_str("\nBlast-radius coverage:\n");
        for row in &plan.coverage {
            let verdict = if row.covered {
                "covered"
            } else {
                "not considered"
            };
            let _ = writeln!(out, "- {} — {}: {}", row.lane, verdict, row.detail);
        }
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
    if !plan.delivery.mrs.is_empty() {
        out.push_str("Merge requests:\n");
        for mr in &plan.delivery.mrs {
            let _ = writeln!(out, "- **MR: {}**", autocode(&mr.title));
            let _ = writeln!(out, "  {}", autocode(&mr.standalone));
            let _ = writeln!(
                out,
                "  {} · needs: {} · satisfies: {}",
                mr.id,
                join_or_dash(mr.needs.iter().map(String::as_str)),
                join_or_dash(mr.satisfies.iter().map(String::as_str))
            );
        }
    }
    out.push('\n');
}

fn md_discussion(out: &mut String, plan: &PlanPresentation) {
    let items = resolved(&plan.comments);
    if items.is_empty() {
        return;
    }
    md_section(out, "Discussion");
    for comment in &items {
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
                    (evidence_map_section(plan))
                    (changed_symbol_section(plan))
                    (call_graph_section(plan))
                    (decision_section(plan))
                    (impact_section(plan))
                    (delivery_section(plan))
                    (timeline_section(plan))
                    (reference_rail(plan))
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
            (section_wrap("Orientation", Fold::Open, markup! {
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
                (section_wrap(&section.heading, Fold::Open, markup! {
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

fn evidence_map_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Evidence map",
        Fold::Open,
        &plan.evidence_claim_map,
        markup! {
            table {
                colgroup { col style="width:62%"; col style="width:38%"; }
                thead { tr { th { "Claim" } th { "Evidence" } } }
                tbody {
                    @for row in &plan.evidence_claim_map {
                        tr {
                            td {
                                div class="item-lead" { strong { (inline_markup(&row.statement)) } }
                                small class="item-cap" { (id_markup(&row.claim)) }
                            }
                            td {
                                @for (index, item) in row.evidence.iter().enumerate() {
                                    @if index > 0 { " " }
                                    span class="pill mark-keep" {
                                        code { (item.id) } " · " (item.method)
                                        @if item.sampled {
                                            " · " span class="pill mark-chg" { "sampled" }
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

fn changed_symbol_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Changed symbols",
        Fold::Open,
        &plan.changed_symbols,
        markup! {
            div class="d tree" {
                @for node in &plan.changed_symbols {
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
    )
}

fn call_graph_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "As-is call graph",
        Fold::Open,
        &plan.call_graph,
        markup! {
            ul class="dg-crossings" {
                @for edge in &plan.call_graph {
                    li { code { (edge.caller) } " → " code { (edge.callee) } }
                }
            }
        },
    )
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
fn timeline_digest(waves: &[TimelineWave]) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "{} {}",
        waves.len(),
        if waves.len() == 1 { "wave" } else { "waves" }
    ));

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
    for wave in waves {
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

/// The retrospective review timeline, folded by default (design §9). The digest
/// carries the all-clear lane/consumer counts; the expanded body spells out only
/// the findings and the exceptions (a contested lane, a missing consumer).
fn timeline_section(plan: &PlanPresentation) -> Markup {
    if plan.timeline.is_empty() {
        return PreEscaped(String::new());
    }
    markup! {
        details class="section secondary timeline-fold" {
            summary {
                h2 { "Review timeline" }
                span class="tl-digest" { (timeline_digest(&plan.timeline)) }
            }
            div class="body" {
                div class="timeline" {
                    @for wave in &plan.timeline {
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
    }
}

#[derive(Clone, Copy)]
enum Fold {
    Open,
    Folded,
}

fn section_wrap(heading: &str, fold: Fold, body: Markup) -> Markup {
    // Consume `body` up front (it is owned, not borrowed) so the parameter earns
    // its by-value signature; re-wrapping the already-escaped string renders
    // identically to splicing the `Markup` directly.
    let body = PreEscaped(body.into_string());
    match fold {
        Fold::Open => markup! {
            section class="section" {
                h2 { (heading) }
                (body)
            }
        },
        Fold::Folded => markup! {
            details class="section secondary" {
                summary { h2 { (heading) } }
                div class="body" { (body) }
            }
        },
    }
}

fn section_when_nonempty<T>(heading: &str, fold: Fold, items: &[T], body: Markup) -> Markup {
    if items.is_empty() {
        PreEscaped(String::new())
    } else {
        section_wrap(heading, fold, body)
    }
}

fn discussion_section(plan: &PlanPresentation) -> Markup {
    let items = resolved(&plan.comments);
    markup! {
        @if !items.is_empty() {
            (section_wrap("Discussion", Fold::Folded, markup! {
                p class="prose" { "Resolved review comments." }
                @for comment in &items {
                    div class="disc" {
                        div class="disc-src" { (discussion_source(plan, comment.anchor)) }
                        blockquote class="revy-comment" { (comment.text.as_str()) }
                    }
                }
            }))
        }
    }
}

fn reference_rail(plan: &PlanPresentation) -> Markup {
    let folded = [
        limitation_section(plan),
        unresolved_section(plan),
        discussion_section(plan),
    ];
    let has_content = folded.iter().any(|section| !section.0.is_empty());
    markup! {
        @if has_content {
            h2 class="rail" { "Reference" }
        }
        @for section in &folded {
            (section)
        }
    }
}

fn limitation_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Evidence limitations",
        Fold::Folded,
        &plan.evidence_limitations,
        markup! {
            ul {
                @for limitation in &plan.evidence_limitations {
                    li { (limitation.source) ": " (inline_markup(&limitation.detail)) }
                }
            }
        },
    )
}

fn unresolved_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Unresolved facts and remedies",
        Fold::Folded,
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

fn coverage_strip(plan: &PlanPresentation) -> Markup {
    markup! {
        @if !plan.coverage.is_empty() {
            div class="coverage" {
                @for row in &plan.coverage {
                    div class=(if row.covered { "cov cov-yes" } else { "cov cov-no" })
                        title=(if row.covered { "covered" } else { "not considered" }) {
                        span class="cov-mark" aria-hidden="true" { (if row.covered { "✓" } else { "—" }) }
                        span class="cov-lane" { (row.lane) }
                        span class="cov-detail" { (inline_markup(&row.detail)) }
                    }
                }
            }
        }
    }
}

fn impact_section(plan: &PlanPresentation) -> Markup {
    if plan.impact_areas.is_empty() && plan.coverage.is_empty() {
        return PreEscaped(String::new());
    }
    section_wrap(
        "Impact areas",
        Fold::Open,
        markup! {
            (coverage_strip(plan))
            @if !plan.impact_areas.is_empty() {
            table {
                thead { tr { th { "Area" } th { "State" } th { "Rationale" } th { "References" } } }
                tbody {
                    @for area in &plan.impact_areas {
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
                        }
                    }
                }
            }
            }
        },
    )
}

fn decision_section(plan: &PlanPresentation) -> Markup {
    section_when_nonempty(
        "Decisions",
        Fold::Open,
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
        Fold::Open,
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
            @if !plan.delivery.mrs.is_empty() {
                h3 { "Merge requests" }
                ul class="lead-list" {
                    @for mr in &plan.delivery.mrs {
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
            @if !plan.delivery.mrs.is_empty() && !plan.authored_delivery_dag {
                figure class="dg-figure" {
                    figcaption { "Merge request dependency graph" }
                    (mr_dag_markup(&plan.delivery.mr_dag))
                }
            }
        },
    )
}

fn mr_dag_markup(dag: &MrDagView) -> Markup {
    markup! {
        div class="d dag mr-dag" {
            @for (index, wave) in dag.waves.iter().enumerate() {
                div class="wv-stage" {
                    span class="wv-dot" {}
                    div class="wv-body" {
                        div class="wv-h" {
                            "stage " (index + 1)
                            span class="wv-par" { (wave.len()) " in parallel" }
                        }
                        div class="wv-mrs" {
                            @for node in wave {
                                div class="mr mr-k-normal" {
                                    a class="mr-id" href=(format!("#mr-{}", node.id)) { "MR:" (node.id) }
                                    span class="mr-t" { (inline_markup(&node.title)) }
                                }
                            }
                        }
                    }
                }
            }
        }
        @if !dag.edges.is_empty() {
            ul class="dg-crossings" {
                @for edge in &dag.edges {
                    li { "MR:" (id_markup(&edge.to)) " needs MR:" (id_markup(&edge.from)) }
                }
            }
        }
    }
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
