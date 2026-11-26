//! Pure workflow transitions.
//!
//! Derive the ready frontier from a proven [`RunState`], and apply a checked
//! result to advance the plan. A rejected result yields a typed [`ApplyError`]
//! whose carried plan is byte-identical to the input.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::capability::{CapabilityDecision, CapabilityReport};
use crate::document::{ParsePlanError, ParsedPlan};
use crate::ids::{DecisionId, EvidenceId, FindingId, ProposalId};
use crate::issues::{Collector, Issues, JsonPointer};
use crate::model::content::{
    Decision, DecisionOwner, DecisionState, DesignBrief, ProposedDecision, Risk, RuledOut,
    SpecialistFocus, SymbolChange, UserAnswer, UserQuestion,
};
use crate::model::impact::{AffectedShape, BlastLane, ImpactEntry, LaneCoverage};
use crate::model::review::{
    CompletedReview, ConsumerVerdict, Finding, LaneVerdictKind, PlanRegion, ReviewEscalation,
    ReviewKind, RulingVerdict, Triage, TriageOutcome,
};
use crate::packet::{
    CheckedRevision, CheckedWorkResult, CorrectionResult, DesignBriefResult, DraftResult,
    ExplorationNext, ExplorationResult, Frontier, ProvisionalRevision, ProvisionalWorkResult,
    RawWorkResult, ReadyFrontier, ReadyPacket, ReviewDecisionResult, RevisionResult,
    UserDecisionResult, WorkKey, WorkKind, WorkSubject,
};
use crate::primitives::{NonEmptyText, OneOrMore, Sha256Digest};
use crate::repository::Repositories;
use crate::state::{
    ApprovedCapabilities, ApprovedCapabilityError, Candidate, CandidateError, DecisionDiff,
    DesignLineage, ExplorationContext, ExplorationContextError, ExplorationPhase,
    ExplorationProgress, PriorDesign, PriorDraft, RawCandidate, RawExplorationContext, Reopen,
    ReopenTarget, ReviewLineage, ReviewRound, Revision, RunState, WaveNumber,
    candidate_fingerprint, history_finding_ids, prove_tool_failures_approved, report_digest,
};

// --- Frontier ------------------------------------------------------------------

/// The ready frontier for a plan.
pub fn frontier(plan: &ParsedPlan) -> Frontier {
    match plan.state() {
        RunState::AwaitingCapabilityApproval {
            report,
            revision_request,
            ..
        } => {
            let approval = ReadyPacket::new(
                WorkKind::CapabilityApproval,
                WorkSubject::Report {
                    digest: report_digest(report),
                },
                mandate(WorkKind::CapabilityApproval),
                approval_context(report),
            );
            let revision = ReadyPacket::new(
                WorkKind::CapabilityReport,
                snapshot_subject(plan),
                mandate(WorkKind::CapabilityReport),
                report_revision_context(revision_request.as_ref()),
            );
            Frontier::Ready(ReadyFrontier::many(OneOrMore::from_first(
                approval,
                vec![revision],
            )))
        }
        RunState::Exploring {
            capabilities,
            progress: ExplorationProgress::Collecting { prior },
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::Exploration,
            exploration_context(prior.raw(), capabilities, plan.repositories()),
        ))),
        RunState::Exploring {
            progress: ExplorationProgress::Complete { context },
            lineage,
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::DesignBrief,
            brief_authoring_context(context, lineage.prior_design.as_ref()),
        ))),
        RunState::AwaitingUserDecision { questions, .. } => {
            Frontier::Ready(ReadyFrontier::single(snapshot_packet(
                plan,
                WorkKind::UserDecision,
                json!({ "questions": questions }),
            )))
        }
        RunState::Drafting {
            context,
            brief,
            decisions,
            lineage,
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::Draft,
            draft_context(brief, decisions, context, lineage.prior_design.as_ref()),
        ))),
        RunState::Reviewing {
            candidate,
            wave,
            pending,
            history,
            ..
        } => Frontier::Ready(review_frontier(candidate, *wave, pending, history)),
        RunState::Correcting {
            candidate,
            reviews,
            must_fix,
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::Correction,
            correction_context(candidate, reviews, must_fix),
        ))),
        RunState::AwaitingReviewDecision {
            unresolved, cause, ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::ReviewDecision,
            json!({ "unresolved": unresolved, "cause": cause }),
        ))),
        RunState::Final { .. } => Frontier::Terminal,
    }
}

/// One review packet per pending kind, all keyed on the candidate fingerprint so
/// sibling review keys stay valid across one apply (design decision 7).
fn review_frontier(
    candidate: &Candidate,
    wave: WaveNumber,
    pending: &OneOrMore<ReviewKind>,
    history: &[ReviewRound],
) -> ReadyFrontier {
    let subject = WorkSubject::Candidate {
        fingerprint: candidate_fingerprint(candidate),
    };
    let packet = |kind: WorkKind| {
        let context = match kind {
            WorkKind::SpecialistReview { focus } => {
                specialist_context(candidate, wave, history, focus)
            }
            _ => review_context(candidate, wave, history),
        };
        ReadyPacket::new(kind, subject.clone(), mandate(kind), context)
    };
    match pending.as_slice() {
        [first, rest @ ..] => {
            let head = packet(review_work_kind(*first));
            let tail = rest.iter().map(|k| packet(review_work_kind(*k))).collect();
            ReadyFrontier::many(OneOrMore::from_first(head, tail))
        }
        // `pending` is a `OneOrMore`, so the empty case is unreachable; fall back
        // to the integrated packet rather than fabricate an empty frontier.
        [] => ReadyFrontier::single(packet(WorkKind::IntegratedReview)),
    }
}

const fn review_work_kind(kind: ReviewKind) -> WorkKind {
    match kind {
        ReviewKind::Integrated => WorkKind::IntegratedReview,
        ReviewKind::Specialist { focus } => WorkKind::SpecialistReview { focus },
    }
}

fn snapshot_subject(plan: &ParsedPlan) -> WorkSubject {
    WorkSubject::RunSnapshot {
        digest: Sha256Digest::hash(&serde_json::to_vec(&plan.state().to_raw()).unwrap_or_default()),
    }
}

fn snapshot_packet(plan: &ParsedPlan, kind: WorkKind, context: Value) -> ReadyPacket {
    ReadyPacket::new(kind, snapshot_subject(plan), mandate(kind), context)
}

// --- Context builders ----------------------------------------------------------

fn approval_context(report: &CapabilityReport) -> Value {
    json!({
        "capabilities": report.capabilities.iter().map(|capability| json!({
            "id": capability.id,
            "name": capability.name,
            "interface": capability.interface,
        })).collect::<Vec<_>>(),
        "proposals": report.proposals.iter().map(|proposal| json!({
            "id": proposal.id,
            "name": proposal.name,
            "side_effects": proposal.side_effects,
            "recommendation": proposal.recommendation,
        })).collect::<Vec<_>>(),
    })
}

fn report_revision_context(revision_request: Option<&NonEmptyText>) -> Value {
    let mut value = json!({
        "note": "supply a revised capability report to replace the current one; the user decides over it",
    });
    if let (Some(instruction), Value::Object(map)) = (revision_request, &mut value) {
        map.insert("revision_request".to_owned(), json!(instruction));
    }
    value
}

fn exploration_context(
    prior: &RawExplorationContext,
    capabilities: &ApprovedCapabilities,
    repositories: &Repositories,
) -> Value {
    let approved_tools: Vec<&NonEmptyText> = capabilities
        .approved()
        .iter()
        .filter_map(|proposal| {
            capabilities
                .report()
                .proposals
                .iter()
                .find(|candidate| &candidate.id == proposal)
                .map(|candidate| &candidate.name)
        })
        .collect();
    json!({
        "prior_evidence_ids": prior.evidence.iter().map(evidence_id_value).collect::<Vec<_>>(),
        "prior_claim_ids": prior.claims.iter().map(|claim| &claim.id).collect::<Vec<_>>(),
        "approved_tools": approved_tools,
        "target_identity": repositories.primary().identity(),
        "primary_repository": repositories.primary_alias(),
        "repositories": repositories_context(repositories),
    })
}

/// Every repository a result may cite, by alias: its root, its identity, and
/// how to cite it (`path` for the primary, `alias:path` otherwise).
fn repositories_context(repositories: &Repositories) -> Value {
    Value::Object(
        repositories
            .iter()
            .map(|repo| {
                let cite = if repo.primary {
                    "path".to_owned()
                } else {
                    format!("{}:path", repo.alias)
                };
                (
                    repo.alias.to_string(),
                    json!({
                        "root": repo.repository.root,
                        "identity": repo.repository.identity(),
                        "cite_as": cite,
                    }),
                )
            })
            .collect(),
    )
}

fn brief_authoring_context(context: &ExplorationContext, prior: Option<&PriorDesign>) -> Value {
    let mut value = json!({
        "claim_ids": context.claims().iter().map(|claim| &claim.id).collect::<Vec<_>>(),
        "impact_subjects": context.impact().iter().map(|a| &a.subject).collect::<Vec<_>>(),
    });
    if let (Some(prior), Value::Object(map)) = (prior, &mut value) {
        map.insert(
            "prior_design".to_owned(),
            json!({
                "brief": prior.brief,
                "decisions": prior.decisions,
                "answers": prior.answers,
                "note": "`--patch` applies to {brief, decisions} of this prior design; a user decision returned with the same question and alternatives keeps its prior answer and is not asked again",
            }),
        );
    }
    value
}

fn draft_context(
    brief: &DesignBrief,
    decisions: &[Decision],
    context: &ExplorationContext,
    prior: Option<&PriorDesign>,
) -> Value {
    let mut value = json!({
        "brief": brief,
        "decisions": decisions,
        "claim_ids": context.claims().iter().map(|claim| &claim.id).collect::<Vec<_>>(),
        "impact_subjects": context.impact().iter().map(|a| &a.subject).collect::<Vec<_>>(),
        "coverage": context.coverage(),
    });
    if let (Some(draft), Value::Object(map)) =
        (prior.and_then(|prior| prior.draft.as_ref()), &mut value)
    {
        map.insert("prior_draft".to_owned(), prior_draft_guide(draft));
    }
    value
}

/// How to answer a reopened draft with `--patch`: the prior draft's
/// addressable ids, the subject shape, and an example.
fn prior_draft_guide(draft: &PriorDraft) -> Value {
    let content = &draft.content;
    let delivery = &draft.delivery;
    json!({
        "ids": {
            "requirements": delivery.requirements.iter().map(|r| &r.id).collect::<Vec<_>>(),
            "acceptance_criteria": delivery.acceptance_criteria.iter().map(|c| &c.id).collect::<Vec<_>>(),
            "risks": content.risks.iter().map(Risk::id).collect::<Vec<_>>(),
            "mrs": delivery.mrs.iter().map(|mr| &mr.id).collect::<Vec<_>>(),
            "changed_symbols": content.changed_symbols.iter().map(|s| &s.symbol).collect::<Vec<_>>(),
            "sections": content.sections.iter().enumerate()
                .map(|(index, section)| json!({ "index": index, "heading": section.heading }))
                .collect::<Vec<_>>(),
        },
        "patch": {
            "subject": { "content": "<the prior draft's content>", "delivery": "<the prior draft's delivery>" },
            "selectors": "a path segment `@<key>` under an array selects the element whose id (or subject, symbol, lane) is <key>; sections are addressed by index",
            "example": [
                { "op": "replace", "path": "/delivery/mrs/@mr-1/title", "value": "Add the bounded queue type" },
                { "op": "add", "path": "/content/risks/-",
                  "value": { "kind": "ordinary", "id": "risk-new", "statement": "…", "mitigation": "…" } }
            ],
        },
    })
}

fn correction_context(
    candidate: &Candidate,
    reviews: &OneOrMore<CompletedReview>,
    must_fix: &[FindingId],
) -> Value {
    let material: Vec<&Finding> = wave_findings(reviews)
        .filter(|finding| finding.is_material())
        .collect();
    json!({
        "material_findings": material,
        "must_fix": must_fix,
        "regions": candidate.regions().iter().collect::<Vec<_>>(),
        "patch": correction_patch_guide(),
    })
}

/// How to answer a correction with `--patch` instead of resending the candidate.
fn correction_patch_guide() -> Value {
    json!({
        "subject": { "candidate": "<the current candidate>", "triage": [] },
        "selectors": "a path segment `@<key>` under an array selects the element whose id (or subject, symbol, lane) is <key>",
        "example": [
            { "op": "replace", "path": "/candidate/content/risks/@risk-cache/statement",
              "value": "The cache is invalidated on every write." },
            { "op": "add", "path": "/triage/-",
              "value": { "finding": "finding-1",
                         "outcome": { "kind": "fix", "affected": [{ "region": "risk", "id": "risk-cache" }] } } }
        ],
    })
}

/// The review packet's context (design §7): the wave, the not-considered lanes to
/// review, the modified/removed symbols to consumer-check, and every finding a
/// prior round rejected (so the reviewer does not re-raise it blindly).
fn review_context(candidate: &Candidate, wave: WaveNumber, history: &[ReviewRound]) -> Value {
    let raw = candidate.raw();
    let not_considered: Vec<Value> = raw
        .exploration
        .coverage
        .iter()
        .filter_map(|coverage| match &coverage.coverage {
            LaneCoverage::NotConsidered { reason } => Some(json!({
                "lane": coverage.lane,
                "reason": reason,
            })),
            LaneCoverage::Covered { .. } => None,
        })
        .collect();
    let consumer_symbols: Vec<Value> = raw
        .content
        .changed_symbols
        .iter()
        .filter(|symbol| matches!(symbol.change, SymbolChange::Modified | SymbolChange::Removed))
        .map(|symbol| json!({ "symbol": symbol.symbol, "path": symbol.path, "change": symbol.change }))
        .collect();
    let prior_rejections = rejected_findings(history);
    json!({
        "wave": wave,
        "not_considered_lanes": not_considered,
        "consumer_symbols": consumer_symbols,
        "prior_rejections": prior_rejections,
        "tool_failures": raw.exploration.tool_failures,
    })
}

/// The specialist review packet's context: the shared review context plus the
/// `focus` this specialist covers, so a driver learns which specialist to run
/// from the packet itself rather than reading it back out of plan.json.
fn specialist_context(
    candidate: &Candidate,
    wave: WaveNumber,
    history: &[ReviewRound],
    focus: SpecialistFocus,
) -> Value {
    let mut context = review_context(candidate, wave, history);
    // `review_context` always returns a JSON object; the focus joins its other
    // fields so the specialist's target sits beside the review context it shares.
    if let Value::Object(map) = &mut context {
        map.insert("focus".to_owned(), json!(focus));
    }
    context
}

/// Every finding a prior round's triage rejected, with its statement and the
/// rationale — so the next reviewer sees the disagreement (design §7).
fn rejected_findings(history: &[ReviewRound]) -> Vec<Value> {
    let mut out = Vec::new();
    for round in history {
        for triage in &round.triage {
            if let TriageOutcome::Reject { rationale } = &triage.outcome {
                let statement = finding_statement(round, &triage.finding);
                out.push(json!({
                    "finding": triage.finding,
                    "statement": statement,
                    "rationale": rationale,
                }));
            }
        }
    }
    out
}

fn finding_statement<'a>(round: &'a ReviewRound, finding: &FindingId) -> Option<&'a NonEmptyText> {
    round
        .reviews
        .iter()
        .flat_map(CompletedReview::findings)
        .find(|candidate| &candidate.id == finding)
        .map(|candidate| &candidate.statement)
}

fn evidence_id_value(evidence: &crate::evidence::Evidence) -> Value {
    use crate::evidence::Evidence;
    let id = match evidence {
        Evidence::Runtime(evidence) => &evidence.id,
        Evidence::Semantic(evidence) => &evidence.id,
        Evidence::Structural(evidence) => &evidence.id,
        Evidence::Textual(evidence) => &evidence.id,
        Evidence::Documentation(evidence) => &evidence.id,
        Evidence::History(evidence) => &evidence.id,
        Evidence::Inference(evidence) => &evidence.id,
    };
    json!(id.as_str())
}

// --- Apply ---------------------------------------------------------------------

/// Apply a checked result to a plan, advancing it to the next state.
pub fn apply(
    plan: ParsedPlan,
    key: &WorkKey,
    result: CheckedWorkResult,
) -> Result<ParsedPlan, ApplyError> {
    if !frontier(&plan).iter().any(|packet| packet.key() == key) {
        return Err(ApplyError::new(plan, ApplyReason::StaleWorkKey.into()));
    }
    match next_state(plan.state(), result.into_raw(), plan.repositories()) {
        Ok(next) => rebuild(&plan, &next).map_err(|error| {
            ApplyError::new(
                plan,
                ApplyReason::Internal {
                    detail: error.to_string(),
                }
                .into(),
            )
        }),
        Err(reasons) => Err(ApplyError::new(plan, reasons)),
    }
}

/// Every transition defect a result that failed its runtime check would also
/// hit, so one `check`/`apply` reports both lists. Never yields a plan: a
/// provisional result cannot advance state.
pub fn rehearse(
    plan: &ParsedPlan,
    key: &WorkKey,
    result: ProvisionalWorkResult,
) -> Result<(), Rejection> {
    if !frontier(plan).iter().any(|packet| packet.key() == key) {
        return Err(ApplyReason::StaleWorkKey.into());
    }
    next_state(plan.state(), result.into_raw(), plan.repositories()).map(|_next| ())
}

/// Every reason a result could not advance the plan, each at a JSON pointer into
/// the result document.
pub type Rejection = Issues<ApplyReason>;

fn at(pointer: String) -> JsonPointer {
    JsonPointer::new(pointer)
}

// One arm per (state, result) pair the workflow admits.
#[allow(clippy::too_many_lines)]
fn next_state(
    state: &RunState,
    result: RawWorkResult,
    repositories: &Repositories,
) -> Result<RunState, Rejection> {
    use RawWorkResult as Result_;
    let result_kind = result_kind_name(&result);
    match (state, result) {
        // A revised report always waits for the user: they asked for the
        // revision, or an exploration proposed the capability change.
        (
            RunState::AwaitingCapabilityApproval { prior, lineage, .. },
            Result_::CapabilityReport(report),
        ) => Ok(RunState::AwaitingCapabilityApproval {
            report: report.stamped(repositories.primary().identity()),
            prior: prior.clone(),
            revision_request: None,
            lineage: lineage.clone(),
        }),
        (
            RunState::AwaitingCapabilityApproval {
                report,
                prior,
                lineage,
                ..
            },
            Result_::CapabilityDecision(decision),
        ) => Ok(next_from_approval(
            report,
            prior.as_ref(),
            lineage,
            &decision,
        )?),
        (
            RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::Collecting { .. },
                lineage,
            },
            Result_::Exploration(result),
        ) => next_from_exploration(capabilities, lineage, result, repositories),
        (
            RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::Complete { context },
                lineage,
            },
            Result_::DesignBrief(result),
        ) => next_from_brief(capabilities, context, lineage, result),
        (
            RunState::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            },
            Result_::UserDecision(result),
        ) => Ok(next_from_user_decision(
            &Asked {
                capabilities,
                context,
                brief,
                decisions,
                questions,
                carried_answers,
                lineage,
            },
            &result,
        )?),
        (
            RunState::Drafting {
                capabilities,
                context,
                brief,
                decisions,
                lineage,
            },
            Result_::Draft(result),
        ) => next_from_draft(
            capabilities,
            context,
            brief,
            decisions,
            lineage,
            result,
            repositories,
        ),
        (
            RunState::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
                lineage,
            },
            Result_::Review(review),
        ) => next_from_review(
            &Under {
                candidate,
                wave: *wave,
                history,
                lineage,
            },
            pending,
            completed,
            review,
        ),
        (
            RunState::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
                lineage,
            },
            Result_::Correction(result),
        ) => next_from_correction(
            &Under {
                candidate,
                wave: *wave,
                history,
                lineage,
            },
            reviews,
            must_fix,
            result,
            repositories,
        ),
        (
            RunState::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                history,
                lineage,
                ..
            },
            Result_::ReviewDecision(result),
        ) => Ok(next_from_review_decision(
            &Under {
                candidate,
                wave: *wave,
                history,
                lineage,
            },
            unresolved,
            &result,
        )?),
        (state, _) => Err(ApplyReason::ResultKindMismatch {
            state: state_name(state),
            kind: result_kind,
        }
        .into()),
    }
}

/// A candidate under review at one wave, with its history and lineage — what
/// every review-loop transition reads.
struct Under<'a> {
    candidate: &'a Candidate,
    wave: WaveNumber,
    history: &'a [ReviewRound],
    lineage: &'a ReviewLineage,
}

/// The open questions a user decision answers, and what the state carries
/// beside them.
struct Asked<'a> {
    capabilities: &'a ApprovedCapabilities,
    context: &'a ExplorationContext,
    brief: &'a DesignBrief,
    decisions: &'a OneOrMore<ProposedDecision>,
    questions: &'a OneOrMore<UserQuestion>,
    carried_answers: &'a [UserAnswer],
    lineage: &'a DesignLineage,
}

fn next_from_approval(
    report: &CapabilityReport,
    prior: Option<&ExplorationContext>,
    lineage: &DesignLineage,
    decision: &CapabilityDecision,
) -> Result<RunState, ApplyReason> {
    if decision
        .declared_report()
        .is_some_and(|declared| *declared != report_digest(report))
    {
        return Err(ApplyReason::DigestMismatch);
    }
    match decision {
        CapabilityDecision::RequestRevision { instruction, .. } => {
            Ok(RunState::AwaitingCapabilityApproval {
                report: report.clone(),
                prior: prior.cloned(),
                revision_request: Some(instruction.clone()),
                lineage: lineage.clone(),
            })
        }
        CapabilityDecision::ProceedCurrent { .. } => {
            let capabilities = ApprovedCapabilities::parse(report.clone(), Vec::new())?;
            Ok(exploring(capabilities, prior, lineage))
        }
        CapabilityDecision::ApproveAdditions { proposals, .. } => {
            let capabilities =
                ApprovedCapabilities::parse(report.clone(), proposals.iter().cloned().collect())?;
            Ok(exploring(capabilities, prior, lineage))
        }
    }
}

fn exploring(
    capabilities: ApprovedCapabilities,
    prior: Option<&ExplorationContext>,
    lineage: &DesignLineage,
) -> RunState {
    let progress = prior.map_or_else(ExplorationProgress::initial, |prior| {
        ExplorationProgress::resuming(prior.clone())
    });
    RunState::Exploring {
        capabilities,
        progress,
        lineage: lineage.clone(),
    }
}

fn next_from_exploration(
    capabilities: &ApprovedCapabilities,
    lineage: &DesignLineage,
    result: ExplorationResult,
    repositories: &Repositories,
) -> Result<RunState, Rejection> {
    let phase = match result.next {
        ExplorationNext::Complete => ExplorationPhase::Complete,
        ExplorationNext::ProposeCapability { .. } => ExplorationPhase::Collecting,
    };
    let context = ExplorationContext::parse(
        RawExplorationContext {
            evidence: result.evidence,
            claims: result.claims,
            uncertainties: result.uncertainties,
            impact: result.impact,
            coverage: result.coverage,
            tool_failures: result.tool_failures,
        },
        repositories,
        capabilities.approved(),
        phase,
    )
    .map_err(|issues| issues.map(ApplyReason::InvalidExploration))?;
    match result.next {
        ExplorationNext::Complete => Ok(RunState::Exploring {
            capabilities: capabilities.clone(),
            progress: ExplorationProgress::Complete { context },
            lineage: lineage.clone(),
        }),
        // A mid-exploration proposal always waits for the user, even when the
        // report would otherwise need no approval: the explorer asked for it.
        ExplorationNext::ProposeCapability { report } => Ok(RunState::AwaitingCapabilityApproval {
            report: report.stamped(repositories.primary().identity()),
            prior: Some(context),
            revision_request: None,
            lineage: lineage.clone(),
        }),
    }
}

fn next_from_brief(
    capabilities: &ApprovedCapabilities,
    context: &ExplorationContext,
    lineage: &DesignLineage,
    result: DesignBriefResult,
) -> Result<RunState, Rejection> {
    let mut duplicates = Collector::new();
    let mut ids: BTreeSet<&DecisionId> = BTreeSet::new();
    for (position, decision) in result.decisions.iter().enumerate() {
        if !ids.insert(decision.id()) {
            duplicates.push(
                at(format!("/decisions/{position}/id")),
                ApplyReason::DuplicateDecision {
                    decision: decision.id().clone(),
                },
            );
        }
    }
    duplicates.finish(())?;
    let carried = carried_answers(lineage.prior_design.as_ref(), &result.decisions);
    let carried_ids: BTreeSet<&DecisionId> = carried.iter().map(|a| &a.decision).collect();
    let questions: Vec<UserQuestion> = result
        .decisions
        .iter()
        .filter_map(|decision| match decision {
            ProposedDecision::User {
                id,
                question,
                alternatives,
                ..
            } if !carried_ids.contains(id) => Some(UserQuestion {
                decision: id.clone(),
                question: question.clone(),
                options: alternatives.clone(),
            }),
            ProposedDecision::User { .. } | ProposedDecision::Planner { .. } => None,
        })
        .collect();
    if let Ok(questions) = OneOrMore::parse(questions) {
        let decisions = OneOrMore::parse(result.decisions).map_err(internal)?;
        Ok(RunState::AwaitingUserDecision {
            capabilities: capabilities.clone(),
            context: context.clone(),
            brief: result.brief,
            decisions,
            questions,
            carried_answers: carried,
            lineage: lineage.clone(),
        })
    } else {
        let decisions = answer_all(&result.decisions, &carried)?;
        Ok(RunState::Drafting {
            capabilities: capabilities.clone(),
            context: context.clone(),
            brief: result.brief,
            decisions,
            lineage: lineage.clone(),
        })
    }
}

/// The prior answer of every user decision that came back with the same
/// question and the same set of alternatives as one the prior design asked.
fn carried_answers(prior: Option<&PriorDesign>, decisions: &[ProposedDecision]) -> Vec<UserAnswer> {
    let Some(prior) = prior else {
        return Vec::new();
    };
    decisions
        .iter()
        .filter_map(|decision| {
            let ProposedDecision::User {
                id,
                question,
                alternatives,
                ..
            } = decision
            else {
                return None;
            };
            let wanted: BTreeSet<&NonEmptyText> = alternatives.iter().collect();
            let answer = prior.decisions.iter().find_map(|old| match old {
                ProposedDecision::User {
                    id: old_id,
                    question: old_question,
                    alternatives: old_alternatives,
                    ..
                } if old_question == question
                    && old_alternatives.iter().collect::<BTreeSet<_>>() == wanted =>
                {
                    prior
                        .answers
                        .iter()
                        .find(|answer| &answer.decision == old_id)
                }
                ProposedDecision::User { .. } | ProposedDecision::Planner { .. } => None,
            })?;
            Some(UserAnswer {
                decision: id.clone(),
                chosen: answer.chosen.clone(),
                note: answer.note.clone(),
            })
        })
        .collect()
}

fn next_from_user_decision(
    asked: &Asked<'_>,
    result: &UserDecisionResult,
) -> Result<RunState, ApplyReason> {
    match result {
        UserDecisionResult::Answer { answers } => {
            let asked_ids: BTreeSet<&DecisionId> = asked
                .questions
                .iter()
                .map(|question| &question.decision)
                .collect();
            let answer_ids: BTreeSet<&DecisionId> =
                answers.iter().map(|answer| &answer.decision).collect();
            if asked_ids != answer_ids {
                return Err(ApplyReason::AnswerMismatch {
                    detail: "answers do not cover exactly the open questions".to_owned(),
                });
            }
            let all: Vec<UserAnswer> = answers
                .iter()
                .chain(asked.carried_answers)
                .cloned()
                .collect();
            let answered = answer_all(asked.decisions.as_slice(), &all)?;
            Ok(RunState::Drafting {
                capabilities: asked.capabilities.clone(),
                context: asked.context.clone(),
                brief: asked.brief.clone(),
                decisions: answered,
                lineage: asked.lineage.clone(),
            })
        }
        UserDecisionResult::ReviseDesign { .. } => Ok(RunState::Exploring {
            capabilities: asked.capabilities.clone(),
            progress: ExplorationProgress::Complete {
                context: asked.context.clone(),
            },
            lineage: asked.lineage.clone(),
        }),
        UserDecisionResult::RequestEvidence { .. } => Ok(RunState::Exploring {
            capabilities: asked.capabilities.clone(),
            progress: ExplorationProgress::resuming(asked.context.clone()),
            lineage: asked.lineage.clone(),
        }),
    }
}

/// Convert every proposed decision into an answered decision, matching each
/// user-owned one to its answer. The answers must cover exactly the user
/// decisions, and each chosen option must be one the question offered (design
/// §6.4).
fn answer_all(
    decisions: &[ProposedDecision],
    answers: &[UserAnswer],
) -> Result<Vec<Decision>, ApplyReason> {
    let user_ids: BTreeSet<&DecisionId> = decisions
        .iter()
        .filter_map(|decision| match decision {
            ProposedDecision::User { id, .. } => Some(id),
            ProposedDecision::Planner { .. } => None,
        })
        .collect();
    let answer_ids: BTreeSet<&DecisionId> = answers.iter().map(|answer| &answer.decision).collect();
    if user_ids != answer_ids {
        return Err(ApplyReason::AnswerMismatch {
            detail: "answers do not cover exactly the open questions".to_owned(),
        });
    }
    let by_id: BTreeMap<&DecisionId, &UserAnswer> = answers
        .iter()
        .map(|answer| (&answer.decision, answer))
        .collect();
    let mut out = Vec::with_capacity(decisions.len());
    for decision in decisions {
        match decision {
            ProposedDecision::User {
                id, alternatives, ..
            } => {
                let answer = by_id
                    .get(id)
                    .copied()
                    .ok_or_else(|| ApplyReason::AnswerMismatch {
                        detail: format!("no answer for decision `{id}`"),
                    })?;
                if !alternatives.iter().any(|option| option == &answer.chosen) {
                    return Err(ApplyReason::AnswerMismatch {
                        detail: format!("chosen option for `{id}` is not one of its alternatives"),
                    });
                }
                out.push(answer_decision(decision, Some(answer)));
            }
            ProposedDecision::Planner { .. } => out.push(answer_decision(decision, None)),
        }
    }
    Ok(out)
}

/// Convert one proposed decision into an answered [`Decision`] (design §6.4).
fn answer_decision(decision: &ProposedDecision, answer: Option<&UserAnswer>) -> Decision {
    match decision {
        ProposedDecision::Planner {
            id,
            question,
            answer: planner_answer,
            consequences,
            ruled_out,
        } => Decision {
            id: id.clone(),
            owner: DecisionOwner::Planner,
            question: question.clone(),
            state: DecisionState::Answered {
                answer: planner_answer.clone(),
                consequences: consequences.clone(),
                ruled_out: ruled_out.clone(),
            },
            note: None,
        },
        ProposedDecision::User {
            id,
            question,
            alternatives,
            consequences,
        } => {
            let chosen = answer.map(|answer| answer.chosen.clone());
            let ruled_out = chosen.as_ref().map_or_else(Vec::new, |chosen| {
                alternatives
                    .iter()
                    .filter(|option| *option != chosen)
                    .map(|option| RuledOut {
                        alternative: option.clone(),
                        reason: NonEmptyText::from_static("not chosen by user"),
                    })
                    .collect()
            });
            Decision {
                id: id.clone(),
                owner: DecisionOwner::User,
                question: question.clone(),
                state: DecisionState::Answered {
                    // A user decision reaching this without an answer is a
                    // programming error; fall back to the question so the
                    // conversion stays total rather than panicking.
                    answer: chosen.unwrap_or_else(|| question.clone()),
                    consequences: consequences.clone(),
                    ruled_out,
                },
                note: answer.and_then(|answer| answer.note.clone()),
            }
        }
    }
}

fn next_from_draft(
    capabilities: &ApprovedCapabilities,
    context: &ExplorationContext,
    brief: &DesignBrief,
    decisions: &[Decision],
    lineage: &DesignLineage,
    result: DraftResult,
    repositories: &Repositories,
) -> Result<RunState, Rejection> {
    let raw = RawCandidate {
        brief: brief.clone(),
        decisions: decisions.to_vec(),
        exploration: context.raw().clone(),
        content: result.content,
        delivery: result.delivery,
    };
    let candidate = Candidate::parse(raw, repositories)
        .map_err(|issues| issues.map(ApplyReason::IncoherentCandidate))?;
    let pending = derive_roster(&candidate);
    Ok(RunState::Reviewing {
        candidate,
        wave: WaveNumber::first(),
        pending,
        completed: Vec::new(),
        history: Vec::new(),
        lineage: ReviewLineage {
            capabilities: Some(capabilities.clone()),
            reopens: lineage.reopens.clone(),
            revisions: Vec::new(),
        },
    })
}

fn next_from_review(
    under: &Under<'_>,
    pending: &OneOrMore<ReviewKind>,
    completed: &[CompletedReview],
    review: CompletedReview,
) -> Result<RunState, Rejection> {
    if !pending.iter().any(|kind| *kind == review.kind()) {
        return Err(ApplyReason::ReviewNotPending {
            kind: review.kind(),
        }
        .into());
    }
    validate_review(&review, under.candidate, under.history)?;
    let done_kind = review.kind();
    let remaining: Vec<ReviewKind> = pending
        .iter()
        .filter(|kind| **kind != done_kind)
        .copied()
        .collect();
    let mut completed = completed.to_vec();
    completed.push(review);
    if let Ok(remaining) = OneOrMore::parse(remaining) {
        Ok(RunState::Reviewing {
            candidate: under.candidate.clone(),
            wave: under.wave,
            pending: remaining,
            completed,
            history: under.history.to_vec(),
            lineage: under.lineage.clone(),
        })
    } else {
        let wave_reviews = OneOrMore::parse(completed).map_err(internal)?;
        Ok(close_wave(under, wave_reviews))
    }
}

/// Validate a completed review before it is accepted (design §7 review ingest),
/// reporting every defect.
fn validate_review(
    review: &CompletedReview,
    candidate: &Candidate,
    history: &[ReviewRound],
) -> Result<(), Rejection> {
    let mut issues = Collector::new();
    let regions = candidate.regions();
    let rejected = history_rejected(history);
    for (position, finding) in review.findings().iter().enumerate() {
        for (region_position, region) in finding.regions.iter().enumerate() {
            if !regions.contains(region) {
                issues.push(
                    at(format!("/findings/{position}/regions/{region_position}")),
                    ApplyReason::UnresolvedRegion {
                        finding: finding.id.clone(),
                        region: region.clone(),
                    },
                );
            }
        }
        if let Some(target) = &finding.reraises
            && !rejected.contains(target)
        {
            issues.push(
                at(format!("/findings/{position}/reraises")),
                ApplyReason::ReraiseUnknown {
                    finding: finding.id.clone(),
                    target: target.clone(),
                },
            );
        }
    }
    if let CompletedReview::Integrated {
        findings,
        lanes,
        consumers,
    } = review
    {
        validate_integrated(candidate, findings, lanes, consumers, &mut issues);
    }
    issues.finish(())
}

/// The integrated review's extra obligations: a verdict for exactly the
/// not-considered lanes, a consumer check for exactly the modified/removed
/// symbols, and a material finding behind every contest or missing consumer.
fn validate_integrated(
    candidate: &Candidate,
    findings: &[Finding],
    lanes: &[crate::model::review::LaneVerdict],
    consumers: &[crate::model::review::ConsumerCheck],
    issues: &mut Collector<ApplyReason>,
) {
    let expected_lanes: BTreeSet<BlastLane> = candidate
        .raw()
        .exploration
        .coverage
        .iter()
        .filter(|coverage| matches!(coverage.coverage, LaneCoverage::NotConsidered { .. }))
        .map(|coverage| coverage.lane)
        .collect();
    let actual_lanes: BTreeSet<BlastLane> = lanes.iter().map(|verdict| verdict.lane).collect();
    if let Some((missing, extra)) = mismatch(&expected_lanes, &actual_lanes, |lane| {
        lane.wire().to_owned()
    }) {
        issues.push(
            at("/lanes".to_owned()),
            ApplyReason::LaneVerdictsMismatch { missing, extra },
        );
    }

    let expected_symbols: BTreeSet<&str> = candidate
        .raw()
        .content
        .changed_symbols
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.change,
                SymbolChange::Modified | SymbolChange::Removed
            )
        })
        .map(|symbol| symbol.symbol.as_str())
        .collect();
    let actual_symbols: BTreeSet<&str> = consumers
        .iter()
        .map(|check| check.symbol.as_str())
        .collect();
    if let Some((missing, extra)) = mismatch(&expected_symbols, &actual_symbols, |symbol| {
        (*symbol).to_owned()
    }) {
        issues.push(
            at("/consumers".to_owned()),
            ApplyReason::ConsumerChecksMismatch { missing, extra },
        );
    }

    let material: BTreeSet<&FindingId> = findings
        .iter()
        .filter(|finding| finding.is_material())
        .map(|finding| &finding.id)
        .collect();
    for (position, verdict) in lanes.iter().enumerate() {
        if let LaneVerdictKind::Contested { finding } = &verdict.verdict
            && !material.contains(finding)
        {
            issues.push(
                at(format!("/lanes/{position}/verdict/finding")),
                ApplyReason::ContestWithoutFinding { lane: verdict.lane },
            );
        }
    }
    // §7: a consumer check that reports a symbol `Missing` must name a
    // material finding in this same review, exactly as a `Contested` lane
    // must. Without this the consumer-check obligation could be reported and
    // then defeated by a nonexistent or advisory finding.
    for (position, check) in consumers.iter().enumerate() {
        if let ConsumerVerdict::Missing { finding } = &check.verdict
            && !material.contains(finding)
        {
            issues.push(
                at(format!("/consumers/{position}/verdict/finding")),
                ApplyReason::MissingWithoutFinding {
                    symbol: check.symbol.clone(),
                },
            );
        }
    }
}

/// The wave-close routing (design §8.2). Appends this wave's round to history
/// (empty triage and rulings), then routes on the material findings. The
/// automatic budget counts waves since the latest revision.
fn close_wave(under: &Under<'_>, reviews: OneOrMore<CompletedReview>) -> RunState {
    let material: Vec<Finding> = wave_findings(&reviews)
        .filter(|finding| finding.is_material())
        .cloned()
        .collect();
    let reraises: Vec<FindingId> = wave_findings(&reviews)
        .filter(|finding| finding.reraises.is_some())
        .map(|finding| finding.id.clone())
        .collect();

    let wave = under.wave;
    let candidate = under.candidate.clone();
    let lineage = under.lineage.clone();
    let mut history = under.history.to_vec();
    history.push(ReviewRound {
        wave,
        reviews: reviews.clone(),
        triage: Vec::new(),
        rulings: Vec::new(),
    });

    match OneOrMore::parse(material) {
        Err(_) => RunState::Final {
            candidate,
            history,
            accepted: lineage.carried_accepted().to_vec(),
            comments: Vec::new(),
            lineage,
        },
        Ok(unresolved) => {
            if !reraises.is_empty() {
                let findings = OneOrMore::parse(reraises)
                    .unwrap_or_else(|_| unresolved.map(|finding| finding.id.clone()));
                RunState::AwaitingReviewDecision {
                    candidate,
                    wave,
                    unresolved,
                    cause: ReviewEscalation::Reraised { findings },
                    history,
                    lineage,
                }
            } else if wave.is_budget_exhausted_since(lineage.budget_base()) {
                RunState::AwaitingReviewDecision {
                    candidate,
                    wave,
                    unresolved,
                    cause: ReviewEscalation::BudgetExhausted,
                    history,
                    lineage,
                }
            } else {
                RunState::Correcting {
                    candidate,
                    wave,
                    reviews,
                    must_fix: Vec::new(),
                    history,
                    lineage,
                }
            }
        }
    }
}

fn next_from_correction(
    under: &Under<'_>,
    reviews: &OneOrMore<CompletedReview>,
    must_fix: &[FindingId],
    result: CorrectionResult,
    repositories: &Repositories,
) -> Result<RunState, Rejection> {
    let candidate = under.candidate;
    let mut issues = Collector::new();
    let candidate_root = at("/candidate".to_owned());
    let corrected_raw = *result.candidate;
    // Region fingerprints exist for any raw candidate, so the fix-region proof
    // still runs (and reports) when the candidate is otherwise incoherent.
    let fingerprinted = Candidate::from_raw(corrected_raw.clone());
    let corrected = issues.absorb(
        Candidate::parse(corrected_raw, repositories)
            .map_err(|candidate_issues| candidate_issues.map(ApplyReason::IncoherentCandidate)),
        &candidate_root,
    );

    // 2. triage covers exactly the wave's material findings.
    let material: BTreeSet<FindingId> = wave_findings(reviews)
        .filter(|finding| finding.is_material())
        .map(|finding| finding.id.clone())
        .collect();
    let triaged: BTreeSet<FindingId> = result
        .triage
        .iter()
        .map(|triage| triage.finding.clone())
        .collect();
    if material != triaged {
        let missing = material.difference(&triaged).cloned().collect();
        let extra = triaged.difference(&material).cloned().collect();
        issues.push(
            at("/triage".to_owned()),
            ApplyReason::TriageMismatch { missing, extra },
        );
    }

    let by_finding: BTreeMap<&FindingId, &Triage> = result
        .triage
        .iter()
        .map(|triage| (&triage.finding, triage))
        .collect();

    // 3. every must_fix id has outcome Fix.
    for finding in must_fix {
        let is_fix = by_finding
            .get(finding)
            .is_some_and(|triage| matches!(triage.outcome, TriageOutcome::Fix { .. }));
        if !is_fix {
            issues.push(
                at("/triage".to_owned()),
                ApplyReason::MustFixRejected {
                    finding: finding.clone(),
                },
            );
        }
    }

    // 4. every Fix region actually changed.
    for (position, triage) in result.triage.iter().enumerate() {
        if let TriageOutcome::Fix { affected } = &triage.outcome {
            for (region_position, region) in affected.iter().enumerate() {
                let before = candidate.fingerprints().get(region);
                let after = fingerprinted.fingerprints().get(region);
                if before == after {
                    issues.push(
                        at(format!(
                            "/triage/{position}/outcome/affected/{region_position}"
                        )),
                        ApplyReason::FixTouchedNothing {
                            finding: triage.finding.clone(),
                            region: region.clone(),
                        },
                    );
                }
            }
        }
    }

    // 5. user-owned decisions keep their answer and alternatives.
    issues.absorb(
        check_user_decisions_unchanged(candidate.raw(), fingerprinted.raw()),
        &candidate_root,
    );
    check_tool_failures(under.lineage, fingerprinted.raw(), &mut issues);

    let corrected = issues.finish(corrected)?;
    let Some(corrected) = corrected else {
        return Err(ApplyReason::Internal {
            detail: "correction candidate rejected without a reported defect".to_owned(),
        }
        .into());
    };

    // 6. record the triage on this wave's round; enter the next wave.
    let history = record_triage(under.history, under.wave, &reviews.clone(), result.triage);
    let pending = derive_roster(&corrected);
    Ok(RunState::Reviewing {
        candidate: corrected,
        wave: under.wave.next(),
        pending,
        completed: Vec::new(),
        history,
        lineage: under.lineage.clone(),
    })
}

/// A replacement candidate's tool failures name only approved proposals. Plans
/// whose review states predate recorded capabilities cannot be checked.
fn check_tool_failures(
    lineage: &ReviewLineage,
    candidate: &RawCandidate,
    issues: &mut Collector<ApplyReason>,
) {
    if let Some(capabilities) = &lineage.capabilities {
        issues.absorb(
            prove_tool_failures_approved(&candidate.exploration, capabilities.approved())
                .map_err(|found| found.map(ApplyReason::InvalidExploration)),
            &at("/candidate/exploration".to_owned()),
        );
    }
}

/// What a correction did to a user-owned decision that it may not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionChange {
    Added,
    Removed,
    Answer,
    Alternatives,
}

impl DecisionChange {
    const fn of(diff: &DecisionDiff) -> Self {
        match diff {
            DecisionDiff::Added { .. } => Self::Added,
            DecisionDiff::Removed { .. } => Self::Removed,
            DecisionDiff::Answer { .. } => Self::Answer,
            DecisionDiff::Alternatives { .. } => Self::Alternatives,
        }
    }
}

impl std::fmt::Display for DecisionChange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Added => "was added",
            Self::Removed => "was removed",
            Self::Answer => "had its `answer` changed",
            Self::Alternatives => {
                "had its alternatives (the answer plus `ruled_out` alternatives) changed"
            }
        })
    }
}

/// A correction may reword a user-owned decision — its question, consequences,
/// or ruled-out reasons — but may not add or remove one, change its answer, or
/// change its set of alternatives (design decision 11). Planner decisions may
/// change freely. Pointers are relative to the corrected candidate.
fn check_user_decisions_unchanged(
    prior: &RawCandidate,
    corrected: &RawCandidate,
) -> Result<(), Rejection> {
    let mut issues = Collector::new();
    for (pointer, diff) in user_decision_diffs(prior, corrected) {
        issues.push(
            pointer,
            ApplyReason::UserDecisionImmutable {
                decision: diff.decision().clone(),
                change: DecisionChange::of(&diff),
            },
        );
    }
    issues.finish(())
}

/// Every change `revised` makes to a user-owned decision's existence, answer,
/// or alternatives, each at its pointer in the revised candidate — the one
/// comparison a correction rejects and a revision records.
fn user_decision_diffs(
    prior: &RawCandidate,
    revised: &RawCandidate,
) -> Vec<(JsonPointer, DecisionDiff)> {
    let sorted = |decision: &Decision| -> Vec<NonEmptyText> {
        decision_alternatives(decision)
            .into_iter()
            .cloned()
            .collect()
    };
    let prior_user: BTreeMap<&DecisionId, &Decision> = user_decisions(prior);
    let revised_ids: BTreeSet<&DecisionId> = user_decisions(revised).into_keys().collect();
    let mut diffs = Vec::new();
    for (position, decision) in revised.decisions.iter().enumerate() {
        if !matches!(decision.owner, DecisionOwner::User) {
            continue;
        }
        let id = decision.id.clone();
        let diff = match prior_user.get(&decision.id) {
            None => Some(DecisionDiff::Added {
                decision: id,
                after: sorted(decision),
            }),
            Some(before) if decision_answer(before) != decision_answer(decision) => {
                Some(DecisionDiff::Answer {
                    decision: id,
                    before: decision_answer(before).cloned(),
                    after: decision_answer(decision).cloned(),
                })
            }
            Some(before) if decision_alternatives(before) != decision_alternatives(decision) => {
                Some(DecisionDiff::Alternatives {
                    decision: id,
                    before: sorted(before),
                    after: sorted(decision),
                })
            }
            Some(_) => None,
        };
        if let Some(diff) = diff {
            diffs.push((at(format!("/decisions/{position}")), diff));
        }
    }
    for (id, before) in &prior_user {
        if !revised_ids.contains(id) {
            diffs.push((
                at("/decisions".to_owned()),
                DecisionDiff::Removed {
                    decision: (*id).clone(),
                    before: sorted(before),
                },
            ));
        }
    }
    diffs
}

const fn decision_answer(decision: &Decision) -> Option<&NonEmptyText> {
    match &decision.state {
        DecisionState::Answered { answer, .. } => Some(answer),
        DecisionState::Proposed => None,
    }
}

fn decision_alternatives(decision: &Decision) -> BTreeSet<&NonEmptyText> {
    match &decision.state {
        DecisionState::Answered {
            answer, ruled_out, ..
        } => std::iter::once(answer)
            .chain(ruled_out.iter().map(|ruled| &ruled.alternative))
            .collect(),
        DecisionState::Proposed => BTreeSet::new(),
    }
}

fn user_decisions(candidate: &RawCandidate) -> BTreeMap<&DecisionId, &Decision> {
    candidate
        .decisions
        .iter()
        .filter(|decision| matches!(decision.owner, DecisionOwner::User))
        .map(|decision| (&decision.id, decision))
        .collect()
}

fn next_from_review_decision(
    under: &Under<'_>,
    unresolved: &OneOrMore<Finding>,
    result: &ReviewDecisionResult,
) -> Result<RunState, ApplyReason> {
    let unresolved_ids: BTreeSet<&FindingId> =
        unresolved.iter().map(|finding| &finding.id).collect();
    let ruled_ids: BTreeSet<&FindingId> = result
        .rulings
        .iter()
        .map(|ruling| &ruling.finding)
        .collect();
    if unresolved_ids != ruled_ids {
        let missing = unresolved_ids
            .difference(&ruled_ids)
            .map(|id| (*id).clone())
            .collect();
        let extra = ruled_ids
            .difference(&unresolved_ids)
            .map(|id| (*id).clone())
            .collect();
        return Err(ApplyReason::RulingsMismatch { missing, extra });
    }

    let history = record_rulings(under.history, under.wave, result.rulings.clone());
    let must_fix: Vec<FindingId> = result
        .rulings
        .iter()
        .filter(|ruling| matches!(ruling.verdict, RulingVerdict::MustFix))
        .map(|ruling| ruling.finding.clone())
        .collect();

    if must_fix.is_empty() {
        let mut accepted: Vec<Finding> = unresolved
            .iter()
            .filter(|finding| {
                result.rulings.iter().any(|ruling| {
                    ruling.finding == finding.id && ruling.verdict == RulingVerdict::Accept
                })
            })
            .cloned()
            .collect();
        // A finding the user just ruled on is settled by that ruling; every
        // other carried acceptance stands.
        accepted.extend(
            under
                .lineage
                .carried_accepted()
                .iter()
                .filter(|carried| !unresolved_ids.contains(&carried.id))
                .cloned(),
        );
        Ok(RunState::Final {
            candidate: under.candidate.clone(),
            history,
            accepted,
            comments: Vec::new(),
            lineage: under.lineage.clone(),
        })
    } else {
        // The wave's reviews are recovered from the round just recorded (appended
        // at wave close).
        let reviews = history
            .last()
            .map(|round| round.reviews.clone())
            .ok_or_else(|| ApplyReason::Internal {
                detail: "review history is empty after recording rulings".to_owned(),
            })?;
        Ok(RunState::Correcting {
            candidate: under.candidate.clone(),
            wave: under.wave,
            reviews,
            must_fix,
            history,
            lineage: under.lineage.clone(),
        })
    }
}

// --- Reopen (B1) ---------------------------------------------------------------

/// Send the plan back to exploration or drafting.
///
/// The approved capabilities are kept, and the design (and draft) it sets aside
/// is carried so the next brief and draft can be patched against them. The set-aside review record is
/// kept on the reopen for the render timeline.
pub fn reopen(
    plan: ParsedPlan,
    target: ReopenTarget,
    reason: NonEmptyText,
) -> Result<ParsedPlan, ApplyError> {
    let next = match target {
        ReopenTarget::Exploration => reopen_exploration(plan.state(), reason),
        ReopenTarget::Draft => reopen_draft(plan.state(), reason),
    };
    match next {
        Ok(next) => rebuild_or_reject(plan, &next),
        Err(reason) => Err(ApplyError::new(plan, reason.into())),
    }
}

/// Reopen exploration on a plan whose repositories were just re-captured.
///
/// `plan` carries the new identities. Every carried evidence item that may
/// omit its `target` has it cleared, so the next exploration apply re-verifies
/// its anchors against the current checkout and stamps the new identity.
pub fn reopen_recaptured(plan: ParsedPlan, reason: NonEmptyText) -> Result<ParsedPlan, ApplyError> {
    match reopen_exploration(plan.state(), reason) {
        Ok(RunState::Exploring {
            capabilities,
            progress,
            lineage,
        }) => {
            let mut context = progress.context().raw().clone();
            for evidence in &mut context.evidence {
                if let Some(slot) = evidence.optional_target_mut() {
                    *slot = None;
                }
            }
            let next = RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::resuming(ExplorationContext::from_raw(context)),
                lineage,
            };
            rebuild_or_reject(plan, &next)
        }
        Ok(other) => rebuild_or_reject(plan, &other),
        Err(reason) => Err(ApplyError::new(plan, reason.into())),
    }
}

fn rebuild_or_reject(plan: ParsedPlan, next: &RunState) -> Result<ParsedPlan, ApplyError> {
    rebuild(&plan, next).map_err(|error| {
        ApplyError::new(
            plan,
            ApplyReason::Internal {
                detail: error.to_string(),
            }
            .into(),
        )
    })
}

/// The parts of a candidate-bearing state a reopen or revision reads.
struct UnderReview<'a> {
    candidate: &'a Candidate,
    history: &'a [ReviewRound],
    lineage: &'a ReviewLineage,
}

fn under_review(state: &RunState) -> Option<UnderReview<'_>> {
    match state {
        RunState::Reviewing {
            candidate,
            history,
            lineage,
            ..
        }
        | RunState::Correcting {
            candidate,
            history,
            lineage,
            ..
        }
        | RunState::AwaitingReviewDecision {
            candidate,
            history,
            lineage,
            ..
        }
        | RunState::Final {
            candidate,
            history,
            lineage,
            ..
        } => Some(UnderReview {
            candidate,
            history,
            lineage,
        }),
        RunState::AwaitingCapabilityApproval { .. }
        | RunState::Exploring { .. }
        | RunState::AwaitingUserDecision { .. }
        | RunState::Drafting { .. } => None,
    }
}

const PREDATES_CAPABILITIES: &str = "this plan's review states predate recorded capabilities, so the approved set to keep is unknown; start a new plan";

fn reopen_exploration(state: &RunState, reason: NonEmptyText) -> Result<RunState, ApplyReason> {
    let unavailable = |detail: &'static str| ApplyReason::ReopenUnavailable {
        target: ReopenTarget::Exploration,
        state: state_name(state),
        detail,
    };
    let (basis, history, revisions) = if let Some(review) = under_review(state) {
        let capabilities = review
            .lineage
            .capabilities
            .clone()
            .ok_or_else(|| unavailable(PREDATES_CAPABILITIES))?;
        let raw = review.candidate.raw();
        let basis = ExplorationBasis {
            capabilities,
            context: ExplorationContext::from_raw(raw.exploration.clone()),
            prior_design: Some(candidate_design(raw)),
            reopens: review.lineage.reopens.clone(),
        };
        (
            basis,
            review.history.to_vec(),
            review.lineage.revisions.clone(),
        )
    } else {
        let basis = pre_candidate_basis(state).ok_or_else(|| {
            unavailable("capabilities are not approved yet; decide the capability report first")
        })?;
        (basis, Vec::new(), Vec::new())
    };
    let mut reopens = basis.reopens;
    reopens.push(Reopen {
        target: ReopenTarget::Exploration,
        reason,
        history,
        revisions,
        comments: state.comments().to_vec(),
    });
    Ok(RunState::Exploring {
        capabilities: basis.capabilities,
        progress: ExplorationProgress::resuming(basis.context),
        lineage: DesignLineage {
            reopens,
            prior_design: basis.prior_design,
        },
    })
}

/// What a reopened exploration resumes from.
struct ExplorationBasis {
    capabilities: ApprovedCapabilities,
    context: ExplorationContext,
    prior_design: Option<PriorDesign>,
    reopens: Vec<Reopen>,
}

/// The basis of a pre-candidate state after capability approval; `None` before
/// approval and in candidate-bearing states.
fn pre_candidate_basis(state: &RunState) -> Option<ExplorationBasis> {
    let carried_draft = |lineage: &DesignLineage| {
        lineage
            .prior_design
            .as_ref()
            .and_then(|prior| prior.draft.clone())
    };
    match state {
        RunState::Exploring {
            capabilities,
            progress,
            lineage,
        } => Some(ExplorationBasis {
            capabilities: capabilities.clone(),
            context: progress.context().clone(),
            prior_design: lineage.prior_design.clone(),
            reopens: lineage.reopens.clone(),
        }),
        RunState::AwaitingUserDecision {
            capabilities,
            context,
            brief,
            decisions,
            carried_answers,
            lineage,
            ..
        } => Some(ExplorationBasis {
            capabilities: capabilities.clone(),
            context: context.clone(),
            prior_design: Some(PriorDesign {
                brief: brief.clone(),
                decisions: decisions.as_slice().to_vec(),
                answers: carried_answers.clone(),
                draft: carried_draft(lineage),
            }),
            reopens: lineage.reopens.clone(),
        }),
        RunState::Drafting {
            capabilities,
            context,
            brief,
            decisions,
            lineage,
        } => Some(ExplorationBasis {
            capabilities: capabilities.clone(),
            context: context.clone(),
            prior_design: Some(prior_design(brief, decisions, carried_draft(lineage))),
            reopens: lineage.reopens.clone(),
        }),
        RunState::AwaitingCapabilityApproval { .. }
        | RunState::Reviewing { .. }
        | RunState::Correcting { .. }
        | RunState::AwaitingReviewDecision { .. }
        | RunState::Final { .. } => None,
    }
}

fn reopen_draft(state: &RunState, reason: NonEmptyText) -> Result<RunState, ApplyReason> {
    let unavailable = |detail: &'static str| ApplyReason::ReopenUnavailable {
        target: ReopenTarget::Draft,
        state: state_name(state),
        detail,
    };
    let review = under_review(state).ok_or_else(|| {
        unavailable(
            "no candidate exists yet; reopen draft needs reviewing, correcting, awaiting_review_decision, or final",
        )
    })?;
    let capabilities = review
        .lineage
        .capabilities
        .clone()
        .ok_or_else(|| unavailable(PREDATES_CAPABILITIES))?;
    let raw = review.candidate.raw();
    let mut reopens = review.lineage.reopens.clone();
    reopens.push(Reopen {
        target: ReopenTarget::Draft,
        reason,
        history: review.history.to_vec(),
        revisions: review.lineage.revisions.clone(),
        comments: state.comments().to_vec(),
    });
    Ok(RunState::Drafting {
        capabilities,
        context: ExplorationContext::from_raw(raw.exploration.clone()),
        brief: raw.brief.clone(),
        decisions: raw.decisions.clone(),
        lineage: DesignLineage {
            reopens,
            prior_design: Some(candidate_design(raw)),
        },
    })
}

/// The candidate a reopen set aside, rebuilt for preview.
///
/// Built from the retained prior design (brief, answered decisions, draft) over
/// the state's current exploration context; `None` when the state carries its own candidate or no
/// reopened draft is retained.
pub fn reopened_preview(state: &RunState) -> Option<Candidate> {
    let (context, lineage) = match state {
        RunState::AwaitingCapabilityApproval { prior, lineage, .. } => (prior.as_ref()?, lineage),
        RunState::Exploring {
            progress, lineage, ..
        } => (progress.context(), lineage),
        RunState::AwaitingUserDecision {
            context, lineage, ..
        }
        | RunState::Drafting {
            context, lineage, ..
        } => (context, lineage),
        RunState::Reviewing { .. }
        | RunState::Correcting { .. }
        | RunState::AwaitingReviewDecision { .. }
        | RunState::Final { .. } => return None,
    };
    let prior = lineage.prior_design.as_ref()?;
    let draft = prior.draft.as_ref()?;
    let decisions = answer_all(&prior.decisions, &prior.answers).ok()?;
    Some(Candidate::from_raw(RawCandidate {
        brief: prior.brief.clone(),
        decisions,
        exploration: context.raw().clone(),
        content: draft.content.clone(),
        delivery: draft.delivery.clone(),
    }))
}

fn candidate_design(raw: &RawCandidate) -> PriorDesign {
    prior_design(
        &raw.brief,
        &raw.decisions,
        Some(PriorDraft {
            content: raw.content.clone(),
            delivery: raw.delivery.clone(),
        }),
    )
}

/// The design behind answered decisions, as the brief proposed them: a planner
/// decision with its answer, a user decision with its alternatives (the answer
/// first, then the ruled-out ones) and the user's answer. A decision still
/// `proposed` carries no answer to keep and is left out.
fn prior_design(
    brief: &DesignBrief,
    decisions: &[Decision],
    draft: Option<PriorDraft>,
) -> PriorDesign {
    let mut proposed = Vec::with_capacity(decisions.len());
    let mut answers = Vec::new();
    for decision in decisions {
        let DecisionState::Answered {
            answer,
            consequences,
            ruled_out,
        } = &decision.state
        else {
            continue;
        };
        match decision.owner {
            DecisionOwner::Planner => proposed.push(ProposedDecision::Planner {
                id: decision.id.clone(),
                question: decision.question.clone(),
                answer: answer.clone(),
                consequences: consequences.clone(),
                ruled_out: ruled_out.clone(),
            }),
            DecisionOwner::User => {
                proposed.push(ProposedDecision::User {
                    id: decision.id.clone(),
                    question: decision.question.clone(),
                    alternatives: OneOrMore::from_first(
                        answer.clone(),
                        ruled_out
                            .iter()
                            .map(|ruled| ruled.alternative.clone())
                            .collect(),
                    ),
                    consequences: consequences.clone(),
                });
                answers.push(UserAnswer {
                    decision: decision.id.clone(),
                    chosen: answer.clone(),
                    note: decision.note.clone(),
                });
            }
        }
    }
    PriorDesign {
        brief: brief.clone(),
        decisions: proposed,
        answers,
        draft,
    }
}

// --- Revise (B3) ---------------------------------------------------------------

/// Replace the candidate under review with a user/orchestrator revision,
/// re-proven, recorded as a [`Revision`], and sent into review at the wave
/// after the last recorded review round.
pub fn revise(plan: ParsedPlan, revision: CheckedRevision) -> Result<ParsedPlan, ApplyError> {
    match next_from_revision(plan.state(), revision.into_inner(), plan.repositories()) {
        Ok(next) => rebuild_or_reject(plan, &next),
        Err(reasons) => Err(ApplyError::new(plan, reasons)),
    }
}

/// Every transition defect a revision that failed its runtime check would also
/// hit. Never yields a plan.
pub fn rehearse_revision(
    plan: &ParsedPlan,
    revision: ProvisionalRevision,
) -> Result<(), Rejection> {
    next_from_revision(plan.state(), revision.into_inner(), plan.repositories()).map(|_next| ())
}

fn next_from_revision(
    state: &RunState,
    revision: RevisionResult,
    repositories: &Repositories,
) -> Result<RunState, Rejection> {
    let Some(review) = under_review(state) else {
        return Err(ApplyReason::RevisionUnavailable {
            state: state_name(state),
        }
        .into());
    };
    let mut history = review.history.to_vec();
    // Reviews of an interrupted wave stay on record; they reviewed the
    // candidate the revision replaces.
    if let RunState::Reviewing {
        wave, completed, ..
    } = state
        && let Ok(reviews) = OneOrMore::parse(completed.clone())
    {
        history.push(ReviewRound {
            wave: *wave,
            reviews,
            triage: Vec::new(),
            rulings: Vec::new(),
        });
    }
    let comments = state.comments().to_vec();
    let wave = history
        .last()
        .map_or_else(WaveNumber::first, |round| round.wave.next());

    let mut issues = Collector::new();
    let root = at("/candidate".to_owned());
    check_tool_failures(review.lineage, &revision.candidate, &mut issues);
    let revised = issues.absorb(
        Candidate::parse(*revision.candidate, repositories)
            .map_err(|candidate_issues| candidate_issues.map(ApplyReason::IncoherentCandidate)),
        &root,
    );
    if revised
        .as_ref()
        .is_some_and(|revised| revised.raw() == review.candidate.raw())
    {
        issues.push(root, ApplyReason::RevisionChangedNothing);
    }
    let known = history_finding_ids(&history);
    for (position, finding) in revision.addresses.iter().enumerate() {
        if !known.contains(finding) {
            issues.push(
                at(format!("/addresses/{position}")),
                ApplyReason::UnknownAddressedFinding {
                    finding: finding.clone(),
                    known: known.iter().cloned().collect(),
                },
            );
        }
    }
    let obligations = revision_obligations(state);
    let missing: Vec<FindingId> = obligations
        .findings
        .iter()
        .filter(|finding| !revision.addresses.contains(finding))
        .cloned()
        .collect();
    if let (false, Some(owed)) = (missing.is_empty(), obligations.owed) {
        issues.push(
            at("/addresses".to_owned()),
            ApplyReason::RevisionMissesObligations { owed, missing },
        );
    }
    let Some(revised) = issues.finish(revised)? else {
        return Err(ApplyReason::Internal {
            detail: "revised candidate rejected without a reported defect".to_owned(),
        }
        .into());
    };
    let carried_accepted = match state {
        RunState::Final { accepted, .. } => accepted.as_slice(),
        _ => review.lineage.carried_accepted(),
    }
    .iter()
    .filter(|finding| !revision.addresses.contains(&finding.id))
    .cloned()
    .collect();
    let decision_changes = user_decision_diffs(review.candidate.raw(), revised.raw())
        .into_iter()
        .map(|(_pointer, diff)| diff)
        .collect();
    let mut lineage = review.lineage.clone();
    lineage.revisions.push(Revision {
        wave,
        note: revision.note,
        addresses: revision.addresses,
        comments,
        cause: obligations.cause,
        carried_accepted,
        decision_changes,
    });
    let pending = derive_roster(&revised);
    Ok(RunState::Reviewing {
        candidate: revised,
        wave,
        pending,
        completed: Vec::new(),
        history,
        lineage,
    })
}

/// What a revision owes the state it revises: the findings it must address
/// (the user's must-fix rulings while correcting, every unresolved finding of
/// an escalation), and the escalation it answers.
struct RevisionObligations {
    owed: Option<OwedFindings>,
    findings: Vec<FindingId>,
    cause: Option<ReviewEscalation>,
}

/// Which findings a revision owes, by the state it revises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwedFindings {
    MustFix,
    Unresolved,
}

impl std::fmt::Display for OwedFindings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MustFix => {
                "a revision of a `correcting` plan must address every finding the user ruled must-fix"
            }
            Self::Unresolved => {
                "a revision of an `awaiting_review_decision` plan must address every unresolved finding"
            }
        })
    }
}

fn revision_obligations(state: &RunState) -> RevisionObligations {
    match state {
        RunState::Correcting { must_fix, .. } => RevisionObligations {
            owed: Some(OwedFindings::MustFix),
            findings: must_fix.clone(),
            cause: None,
        },
        RunState::AwaitingReviewDecision {
            unresolved, cause, ..
        } => RevisionObligations {
            owed: Some(OwedFindings::Unresolved),
            findings: unresolved
                .iter()
                .map(|finding| finding.id.clone())
                .collect(),
            cause: Some(cause.clone()),
        },
        _ => RevisionObligations {
            owed: None,
            findings: Vec::new(),
            cause: None,
        },
    }
}

// --- Roster (design §8.1) ------------------------------------------------------

/// The reviews a candidate requires: the integrated review, plus one specialist
/// per impact-shape trigger and per authored `Risk::Specialist`.
fn derive_roster(candidate: &Candidate) -> OneOrMore<ReviewKind> {
    let raw = candidate.raw();
    let entries: BTreeMap<&NonEmptyText, &ImpactEntry> = raw
        .exploration
        .impact
        .iter()
        .map(|assessment| (&assessment.subject, &assessment.entry))
        .collect();
    let mut specialists: BTreeSet<SpecialistFocus> = BTreeSet::new();
    for coverage in &raw.exploration.coverage {
        let LaneCoverage::Covered { subjects } = &coverage.coverage else {
            continue;
        };
        let named: Vec<&ImpactEntry> = subjects
            .iter()
            .filter_map(|subject| entries.get(subject).copied())
            .collect();
        let any_affected = named
            .iter()
            .any(|entry| matches!(entry, ImpactEntry::Affected { .. }));
        let any_behavioral = named.iter().any(|entry| {
            matches!(
                entry,
                ImpactEntry::Affected {
                    shape: AffectedShape::Behavioral,
                    ..
                }
            )
        });
        let any_additive_or_behavioral = named.iter().any(|entry| {
            matches!(
                entry,
                ImpactEntry::Affected {
                    shape: AffectedShape::Additive | AffectedShape::Behavioral,
                    ..
                }
            )
        });
        match coverage.lane {
            BlastLane::Concurrency if any_behavioral => {
                specialists.insert(SpecialistFocus::Concurrency);
            }
            BlastLane::DataAtRest if any_affected => {
                specialists.insert(SpecialistFocus::Migration);
            }
            BlastLane::Security if any_affected => {
                specialists.insert(SpecialistFocus::Security);
            }
            BlastLane::Compatibility if any_behavioral => {
                specialists.insert(SpecialistFocus::ProtocolCompatibility);
            }
            BlastLane::ResourceBounds if any_affected => {
                specialists.insert(SpecialistFocus::Performance);
            }
            BlastLane::Api if any_additive_or_behavioral => {
                specialists.insert(SpecialistFocus::PublicApi);
            }
            _ => {}
        }
    }
    for risk in &raw.content.risks {
        if let Risk::Specialist { focus, .. } = risk {
            specialists.insert(*focus);
        }
    }
    let rest: Vec<ReviewKind> = specialists
        .into_iter()
        .map(|focus| ReviewKind::Specialist { focus })
        .collect();
    OneOrMore::from_first(ReviewKind::Integrated, rest)
}

// --- History helpers -----------------------------------------------------------

/// The findings that a prior round's triage rejected — the id-space a `reraises`
/// may name (design §8.2).
fn history_rejected(history: &[ReviewRound]) -> BTreeSet<FindingId> {
    history
        .iter()
        .flat_map(|round| &round.triage)
        .filter(|triage| matches!(triage.outcome, TriageOutcome::Reject { .. }))
        .map(|triage| triage.finding.clone())
        .collect()
}

/// Set the triage on this wave's round (appended at close), or append a fresh
/// round if none matches (design §8.3 step 6).
fn record_triage(
    history: &[ReviewRound],
    wave: WaveNumber,
    reviews: &OneOrMore<CompletedReview>,
    triage: Vec<Triage>,
) -> Vec<ReviewRound> {
    let mut history = history.to_vec();
    match history.last_mut() {
        Some(round) if round.wave == wave => round.triage = triage,
        _ => history.push(ReviewRound {
            wave,
            reviews: reviews.clone(),
            triage,
            rulings: Vec::new(),
        }),
    }
    history
}

/// Set the rulings on this wave's round (design §8.4).
fn record_rulings(
    history: &[ReviewRound],
    wave: WaveNumber,
    rulings: Vec<crate::model::review::Ruling>,
) -> Vec<ReviewRound> {
    let mut history = history.to_vec();
    if let Some(round) = history.last_mut()
        && round.wave == wave
    {
        round.rulings = rulings;
    }
    history
}

fn wave_findings(reviews: &OneOrMore<CompletedReview>) -> impl Iterator<Item = &Finding> {
    reviews.iter().flat_map(CompletedReview::findings)
}

// --- Rebuild -------------------------------------------------------------------

fn rebuild(plan: &ParsedPlan, state: &RunState) -> Result<ParsedPlan, ParsePlanError> {
    let document = plan.raw_document(state.to_raw());
    let bytes = serde_json::to_vec(&document).unwrap_or_default();
    ParsedPlan::parse(&bytes)
}

fn mandate(kind: WorkKind) -> NonEmptyText {
    NonEmptyText::from_static(match kind {
        WorkKind::CapabilityReport => {
            "Report every available capability and every proposed addition with its cost and disclosed side effects; propose every missing capability that can be added (a language server for an uncovered language, a code-graph tool), so the report never looks complete by omission. At `init` only, a report with no proposals and a capability proving `semantic` evidence is approved automatically; a revised report always waits for the user."
        }
        WorkKind::CapabilityApproval => {
            "Record the user's decision over the disclosed capability report."
        }
        WorkKind::Exploration => "Explore the affected code and record evidence-backed facts.",
        WorkKind::DesignBrief => "Write the design brief and propose the decisions it turns on.",
        WorkKind::UserDecision => "Record the user's answer to each open question.",
        WorkKind::Draft => "Author the full plan candidate from the checked facts.",
        WorkKind::IntegratedReview => {
            "Run the integrated review: return findings, a verdict on every not-considered lane, and a consumer check for every modified or removed symbol."
        }
        WorkKind::SpecialistReview { .. } => {
            "Run the selected specialist review and return its findings."
        }
        WorkKind::Correction => {
            "Correct the candidate and triage every material finding as fixed in named regions or rejected with a rationale."
        }
        WorkKind::ReviewDecision => "Record the user's ruling on each escalated finding.",
    })
}

const fn state_name(state: &RunState) -> &'static str {
    match state {
        RunState::AwaitingCapabilityApproval { .. } => "awaiting_capability_approval",
        RunState::Exploring { .. } => "exploring",
        RunState::AwaitingUserDecision { .. } => "awaiting_user_decision",
        RunState::Drafting { .. } => "drafting",
        RunState::Reviewing { .. } => "reviewing",
        RunState::Correcting { .. } => "correcting",
        RunState::AwaitingReviewDecision { .. } => "awaiting_review_decision",
        RunState::Final { .. } => "final",
    }
}

const fn result_kind_name(result: &RawWorkResult) -> &'static str {
    match result {
        RawWorkResult::CapabilityReport(_) => "capability_report",
        RawWorkResult::CapabilityDecision(_) => "capability_approval",
        RawWorkResult::Exploration(_) => "exploration",
        RawWorkResult::DesignBrief(_) => "design_brief",
        RawWorkResult::UserDecision(_) => "user_decision",
        RawWorkResult::Draft(_) => "draft",
        RawWorkResult::Review(_) => "review",
        RawWorkResult::Correction(_) => "correction",
        RawWorkResult::ReviewDecision(_) => "review_decision",
    }
}

/// Compute a symmetric set difference, returning `(missing, extra)` when the two
/// sets differ, mapped through `label`.
fn mismatch<T: Ord, F: Fn(&T) -> String>(
    expected: &BTreeSet<T>,
    actual: &BTreeSet<T>,
    label: F,
) -> Option<(Vec<String>, Vec<String>)> {
    let missing: Vec<String> = expected.difference(actual).map(&label).collect();
    let extra: Vec<String> = actual.difference(expected).map(&label).collect();
    if missing.is_empty() && extra.is_empty() {
        None
    } else {
        Some((missing, extra))
    }
}

fn internal(_error: crate::primitives::PrimitiveError) -> ApplyReason {
    ApplyReason::Internal {
        detail: "unexpected empty collection in transition".to_owned(),
    }
}

fn error_text(error: &impl std::fmt::Display) -> String {
    error.to_string()
}

// --- Errors --------------------------------------------------------------------

/// Why a checked result could not advance the plan.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApplyReason {
    #[error("result carries a stale work key")]
    StaleWorkKey,
    #[error("a `{kind}` result does not apply to the `{state}` state")]
    ResultKindMismatch {
        state: &'static str,
        kind: &'static str,
    },
    #[error("review kind {kind:?} is not pending")]
    ReviewNotPending { kind: ReviewKind },
    #[error("finding `{finding}` names region `{region:?}`, which the candidate does not carry")]
    UnresolvedRegion {
        finding: FindingId,
        region: PlanRegion,
    },
    #[error("finding `{finding}` re-raises `{target}`, which no prior triage rejected")]
    ReraiseUnknown {
        finding: FindingId,
        target: FindingId,
    },
    #[error(
        "lane verdicts do not cover exactly the not-considered lanes (missing {missing:?}, extra {extra:?})"
    )]
    LaneVerdictsMismatch {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error(
        "consumer checks do not cover exactly the modified/removed symbols (missing {missing:?}, extra {extra:?})"
    )]
    ConsumerChecksMismatch {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error("lane `{}` is contested without naming a material finding in this review", lane.wire())]
    ContestWithoutFinding { lane: BlastLane },
    #[error(
        "consumer check for `{symbol}` reports the symbol missing without naming a material finding in this review"
    )]
    MissingWithoutFinding { symbol: NonEmptyText },
    #[error("the design brief proposes decision `{decision}` more than once")]
    DuplicateDecision { decision: DecisionId },
    #[error(
        "triage does not cover exactly the wave's material findings (missing {missing:?}, extra {extra:?})"
    )]
    TriageMismatch {
        missing: Vec<FindingId>,
        extra: Vec<FindingId>,
    },
    #[error("finding `{finding}` was ruled must-fix but triaged as rejected")]
    MustFixRejected { finding: FindingId },
    #[error(
        "fix for finding `{finding}` claims region `{region:?}` but that region did not change"
    )]
    FixTouchedNothing {
        finding: FindingId,
        region: PlanRegion,
    },
    #[error(
        "user-owned decision `{decision}` {change} by a correction; a correction may reword a user decision's question, consequences, or ruled-out reasons, never its answer or alternatives — those change only with the user's approval: `revise` records the change for the user to see"
    )]
    UserDecisionImmutable {
        decision: DecisionId,
        change: DecisionChange,
    },
    #[error("cannot reopen {} from `{state}`: {detail}", target.name())]
    ReopenUnavailable {
        target: ReopenTarget,
        state: &'static str,
        detail: &'static str,
    },
    #[error(
        "revise applies to reviewing, correcting, awaiting_review_decision, or final; the plan is `{state}`"
    )]
    RevisionUnavailable { state: &'static str },
    #[error("the revised candidate is identical to the current one; a revision must change it")]
    RevisionChangedNothing,
    #[error(
        "--addresses names finding `{finding}`, which no review of this candidate raised (known: {known:?})"
    )]
    UnknownAddressedFinding {
        finding: FindingId,
        known: Vec<FindingId>,
    },
    #[error("{owed}; add --addresses {}", missing.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "))]
    RevisionMissesObligations {
        owed: OwedFindings,
        missing: Vec<FindingId>,
    },
    #[error("user answers are not admissible: {detail}")]
    AnswerMismatch { detail: String },
    #[error(
        "rulings do not cover exactly the unresolved findings (missing {missing:?}, extra {extra:?})"
    )]
    RulingsMismatch {
        missing: Vec<FindingId>,
        extra: Vec<FindingId>,
    },
    #[error("capability decision digest does not match the report")]
    DigestMismatch,
    #[error("approval names undisclosed proposal `{proposal}`")]
    UndisclosedProposal { proposal: ProposalId },
    #[error("exploration result is not internally consistent: {0}")]
    InvalidExploration(ExplorationContextError),
    #[error("candidate is not coherent: {0}")]
    IncoherentCandidate(CandidateError),
    #[error("internal transition error: {detail}")]
    Internal { detail: String },
}

impl ApplyReason {
    /// The evidence this reason reports as lacking a target, if that is what
    /// it reports.
    pub const fn ungrounded_evidence(&self) -> Option<&EvidenceId> {
        match self {
            Self::InvalidExploration(ExplorationContextError::Ungrounded { id, .. })
            | Self::IncoherentCandidate(CandidateError::Exploration(
                ExplorationContextError::Ungrounded { id, .. },
            )) => Some(id),
            _ => None,
        }
    }
}

impl From<ApplyReason> for Issues<ApplyReason> {
    /// A transition-wide reason, located at the result root.
    fn from(reason: ApplyReason) -> Self {
        Self::one(JsonPointer::root(), reason)
    }
}

impl From<ApprovedCapabilityError> for ApplyReason {
    fn from(error: ApprovedCapabilityError) -> Self {
        match error {
            ApprovedCapabilityError::UndisclosedProposal { proposal } => {
                Self::UndisclosedProposal { proposal }
            }
            ApprovedCapabilityError::AutoApprovalNotAdmissible => Self::Internal {
                detail: error_text(&ApprovedCapabilityError::AutoApprovalNotAdmissible),
            },
        }
    }
}

/// A rejected transition, carrying every reason and the unchanged input plan.
#[derive(Debug)]
pub struct ApplyError {
    plan: Box<ParsedPlan>,
    reasons: Rejection,
}

impl ApplyError {
    fn new(plan: ParsedPlan, reasons: Rejection) -> Self {
        Self {
            plan: Box::new(plan),
            reasons,
        }
    }

    /// The first reason found.
    pub fn reason(&self) -> &ApplyReason {
        self.reasons.first().error()
    }

    /// Every reason, each located in the result document.
    pub const fn reasons(&self) -> &Rejection {
        &self.reasons
    }

    pub fn plan(&self) -> &ParsedPlan {
        &self.plan
    }

    pub fn into_plan(self) -> ParsedPlan {
        *self.plan
    }
}

impl std::fmt::Display for ApplyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.reasons)
    }
}

impl std::error::Error for ApplyError {}
