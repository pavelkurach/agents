//! Pure workflow transitions.
//!
//! Derive the ready frontier from a proven [`RunState`], and apply a checked
//! result to advance the plan. A rejected result yields a typed [`ApplyError`]
//! whose carried plan is byte-identical to the input.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use crate::capability::{CapabilityDecision, CapabilityReport};
use crate::document::{ParsePlanError, ParsedPlan, RawPlanDocument};
use crate::ids::{DecisionId, FindingId, ProposalId};
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
    CheckedWorkResult, CorrectionResult, DesignBriefResult, DraftResult, ExplorationNext,
    ExplorationResult, Frontier, RawWorkResult, ReadyFrontier, ReadyPacket, ReviewDecisionResult,
    UserDecisionResult, WorkKey, WorkKind, WorkSubject,
};
use crate::primitives::{NonEmptyText, OneOrMore, Sha256Digest};
use crate::repository::Repository;
use crate::state::{
    ApprovedCapabilities, ApprovedCapabilityError, Candidate, CandidateError, ExplorationContext,
    ExplorationContextError, ExplorationProgress, RawCandidate, RawExplorationContext, ReviewRound,
    RunState, WaveNumber, candidate_fingerprint, report_digest,
};

// --- Frontier ------------------------------------------------------------------

/// The ready frontier for a plan.
pub fn frontier(plan: &ParsedPlan) -> Frontier {
    match plan.state() {
        RunState::AwaitingCapabilityApproval { report, .. } => {
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
                json!({ "note": "supply a revised capability report to replace the current one" }),
            );
            Frontier::Ready(ReadyFrontier::many(OneOrMore::from_first(
                approval,
                vec![revision],
            )))
        }
        RunState::Exploring {
            capabilities,
            progress: ExplorationProgress::Collecting { prior },
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::Exploration,
            exploration_context(prior.raw(), capabilities),
        ))),
        RunState::Exploring {
            progress: ExplorationProgress::Complete { context },
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::DesignBrief,
            brief_authoring_context(context),
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
            ..
        } => Frontier::Ready(ReadyFrontier::single(snapshot_packet(
            plan,
            WorkKind::Draft,
            draft_context(brief, decisions, context),
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

fn exploration_context(
    prior: &RawExplorationContext,
    capabilities: &ApprovedCapabilities,
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
    })
}

fn brief_authoring_context(context: &ExplorationContext) -> Value {
    json!({
        "claim_ids": context.claims().iter().map(|claim| &claim.id).collect::<Vec<_>>(),
        "impact_subjects": context.impact().iter().map(|a| &a.subject).collect::<Vec<_>>(),
    })
}

fn draft_context(
    brief: &DesignBrief,
    decisions: &[Decision],
    context: &ExplorationContext,
) -> Value {
    json!({
        "brief": brief,
        "decisions": decisions,
        "claim_ids": context.claims().iter().map(|claim| &claim.id).collect::<Vec<_>>(),
        "impact_subjects": context.impact().iter().map(|a| &a.subject).collect::<Vec<_>>(),
        "coverage": context.coverage(),
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
        return Err(ApplyError::new(plan, ApplyReason::StaleWorkKey));
    }
    match next_state(plan.state(), result.into_raw(), plan.repository()) {
        Ok(next) => rebuild(&plan, &next).map_err(|error| {
            ApplyError::new(
                plan,
                ApplyReason::Internal {
                    detail: error.to_string(),
                },
            )
        }),
        Err(reason) => Err(ApplyError::new(plan, reason)),
    }
}

fn next_state(
    state: &RunState,
    result: RawWorkResult,
    repository: &Repository,
) -> Result<RunState, ApplyReason> {
    use RawWorkResult as Result_;
    let result_kind = result_kind_name(&result);
    match (state, result) {
        (RunState::AwaitingCapabilityApproval { prior, .. }, Result_::CapabilityReport(report)) => {
            Ok(RunState::AwaitingCapabilityApproval {
                report,
                prior: prior.clone(),
            })
        }
        (
            RunState::AwaitingCapabilityApproval { report, prior },
            Result_::CapabilityDecision(decision),
        ) => next_from_approval(report, prior.as_ref(), &decision),
        (
            RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::Collecting { .. },
            },
            Result_::Exploration(result),
        ) => next_from_exploration(capabilities, result),
        (
            RunState::Exploring {
                capabilities,
                progress: ExplorationProgress::Complete { context },
            },
            Result_::DesignBrief(result),
        ) => next_from_brief(capabilities, context, result),
        (
            RunState::AwaitingUserDecision {
                capabilities,
                context,
                brief,
                decisions,
                ..
            },
            Result_::UserDecision(result),
        ) => next_from_user_decision(capabilities, context, brief, decisions, &result),
        (
            RunState::Drafting {
                context,
                brief,
                decisions,
                ..
            },
            Result_::Draft(result),
        ) => next_from_draft(context, brief, decisions, result, repository),
        (
            RunState::Reviewing {
                candidate,
                wave,
                pending,
                completed,
                history,
            },
            Result_::Review(review),
        ) => next_from_review(candidate, *wave, pending, completed, history, review),
        (
            RunState::Correcting {
                candidate,
                wave,
                reviews,
                must_fix,
                history,
            },
            Result_::Correction(result),
        ) => next_from_correction(
            candidate, *wave, reviews, must_fix, history, result, repository,
        ),
        (
            RunState::AwaitingReviewDecision {
                candidate,
                wave,
                unresolved,
                history,
                ..
            },
            Result_::ReviewDecision(result),
        ) => next_from_review_decision(candidate, *wave, unresolved, history, &result),
        (state, _) => Err(ApplyReason::ResultKindMismatch {
            state: state_name(state),
            kind: result_kind,
        }),
    }
}

fn next_from_approval(
    report: &CapabilityReport,
    prior: Option<&ExplorationContext>,
    decision: &CapabilityDecision,
) -> Result<RunState, ApplyReason> {
    let declared = match decision {
        CapabilityDecision::ProceedCurrent { report }
        | CapabilityDecision::ApproveAdditions { report, .. }
        | CapabilityDecision::RequestRevision { report, .. } => report,
    };
    if *declared != report_digest(report) {
        return Err(ApplyReason::DigestMismatch);
    }
    match decision {
        CapabilityDecision::RequestRevision { .. } => Ok(RunState::AwaitingCapabilityApproval {
            report: report.clone(),
            prior: prior.cloned(),
        }),
        CapabilityDecision::ProceedCurrent { .. } => {
            let capabilities = ApprovedCapabilities::parse(report.clone(), Vec::new())?;
            Ok(exploring(capabilities, prior))
        }
        CapabilityDecision::ApproveAdditions { proposals, .. } => {
            let capabilities =
                ApprovedCapabilities::parse(report.clone(), proposals.iter().cloned().collect())?;
            Ok(exploring(capabilities, prior))
        }
    }
}

fn exploring(capabilities: ApprovedCapabilities, prior: Option<&ExplorationContext>) -> RunState {
    let progress = prior.map_or_else(ExplorationProgress::initial, |prior| {
        ExplorationProgress::resuming(prior.clone())
    });
    RunState::Exploring {
        capabilities,
        progress,
    }
}

fn next_from_exploration(
    capabilities: &ApprovedCapabilities,
    result: ExplorationResult,
) -> Result<RunState, ApplyReason> {
    let context = ExplorationContext::parse(RawExplorationContext {
        evidence: result.evidence,
        claims: result.claims,
        uncertainties: result.uncertainties,
        impact: result.impact,
        coverage: result.coverage,
    })?;
    match result.next {
        ExplorationNext::Complete => Ok(RunState::Exploring {
            capabilities: capabilities.clone(),
            progress: ExplorationProgress::Complete { context },
        }),
        ExplorationNext::ProposeCapability { report } => Ok(RunState::AwaitingCapabilityApproval {
            report,
            prior: Some(context),
        }),
    }
}

fn next_from_brief(
    capabilities: &ApprovedCapabilities,
    context: &ExplorationContext,
    result: DesignBriefResult,
) -> Result<RunState, ApplyReason> {
    let mut ids: BTreeSet<&DecisionId> = BTreeSet::new();
    for decision in &result.decisions {
        if !ids.insert(decision.id()) {
            return Err(ApplyReason::DuplicateDecision {
                decision: decision.id().clone(),
            });
        }
    }
    let has_user_decision = result
        .decisions
        .iter()
        .any(|decision| matches!(decision, ProposedDecision::User { .. }));
    if has_user_decision {
        let questions: Vec<UserQuestion> = result
            .decisions
            .iter()
            .filter_map(|decision| match decision {
                ProposedDecision::User {
                    id,
                    question,
                    alternatives,
                    ..
                } => Some(UserQuestion {
                    decision: id.clone(),
                    question: question.clone(),
                    options: alternatives.clone(),
                }),
                ProposedDecision::Planner { .. } => None,
            })
            .collect();
        let decisions = OneOrMore::parse(result.decisions).map_err(internal)?;
        let questions = OneOrMore::parse(questions).map_err(internal)?;
        Ok(RunState::AwaitingUserDecision {
            capabilities: capabilities.clone(),
            context: context.clone(),
            brief: result.brief,
            decisions,
            questions,
        })
    } else {
        let decisions = result
            .decisions
            .iter()
            .map(|decision| answer_decision(decision, None))
            .collect();
        Ok(RunState::Drafting {
            capabilities: capabilities.clone(),
            context: context.clone(),
            brief: result.brief,
            decisions,
        })
    }
}

fn next_from_user_decision(
    capabilities: &ApprovedCapabilities,
    context: &ExplorationContext,
    brief: &DesignBrief,
    decisions: &OneOrMore<ProposedDecision>,
    result: &UserDecisionResult,
) -> Result<RunState, ApplyReason> {
    match result {
        UserDecisionResult::Answer { answers } => {
            let answered = answer_all(decisions, answers)?;
            Ok(RunState::Drafting {
                capabilities: capabilities.clone(),
                context: context.clone(),
                brief: brief.clone(),
                decisions: answered,
            })
        }
        UserDecisionResult::ReviseDesign { .. } => Ok(RunState::Exploring {
            capabilities: capabilities.clone(),
            progress: ExplorationProgress::Complete {
                context: context.clone(),
            },
        }),
        UserDecisionResult::RequestEvidence { .. } => Ok(RunState::Exploring {
            capabilities: capabilities.clone(),
            progress: ExplorationProgress::resuming(context.clone()),
        }),
    }
}

/// Convert every proposed decision into an answered decision, matching each
/// user-owned one to its answer. The answers must cover exactly the user
/// questions, and each chosen option must be one the question offered (design
/// §6.4).
fn answer_all(
    decisions: &OneOrMore<ProposedDecision>,
    answers: &OneOrMore<UserAnswer>,
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
    let mut out = Vec::with_capacity(decisions.as_slice().len());
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
            }
        }
    }
}

fn next_from_draft(
    context: &ExplorationContext,
    brief: &DesignBrief,
    decisions: &[Decision],
    result: DraftResult,
    repository: &Repository,
) -> Result<RunState, ApplyReason> {
    let raw = RawCandidate {
        brief: brief.clone(),
        decisions: decisions.to_vec(),
        exploration: context.raw().clone(),
        content: result.content,
        delivery: result.delivery,
    };
    let candidate = Candidate::parse(raw, repository)?;
    let pending = derive_roster(&candidate);
    Ok(RunState::Reviewing {
        candidate,
        wave: WaveNumber::first(),
        pending,
        completed: Vec::new(),
        history: Vec::new(),
    })
}

fn next_from_review(
    candidate: &Candidate,
    wave: WaveNumber,
    pending: &OneOrMore<ReviewKind>,
    completed: &[CompletedReview],
    history: &[ReviewRound],
    review: CompletedReview,
) -> Result<RunState, ApplyReason> {
    if !pending.iter().any(|kind| *kind == review.kind()) {
        return Err(ApplyReason::ReviewNotPending {
            kind: review.kind(),
        });
    }
    validate_review(&review, candidate, history)?;
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
            candidate: candidate.clone(),
            wave,
            pending: remaining,
            completed,
            history: history.to_vec(),
        })
    } else {
        let wave_reviews = OneOrMore::parse(completed).map_err(internal)?;
        Ok(close_wave(candidate, wave, wave_reviews, history))
    }
}

/// Validate a completed review before it is accepted (design §7 review ingest).
fn validate_review(
    review: &CompletedReview,
    candidate: &Candidate,
    history: &[ReviewRound],
) -> Result<(), ApplyReason> {
    let regions = candidate.regions();
    let rejected = history_rejected(history);
    for finding in review.findings() {
        for region in &finding.regions {
            if !regions.contains(region) {
                return Err(ApplyReason::UnresolvedRegion {
                    finding: finding.id.clone(),
                    region: region.clone(),
                });
            }
        }
        if let Some(target) = &finding.reraises
            && !rejected.contains(target)
        {
            return Err(ApplyReason::ReraiseUnknown {
                finding: finding.id.clone(),
                target: target.clone(),
            });
        }
    }
    if let CompletedReview::Integrated {
        findings,
        lanes,
        consumers,
    } = review
    {
        let expected_lanes: BTreeSet<BlastLane> = candidate
            .raw()
            .exploration
            .coverage
            .iter()
            .filter(|coverage| matches!(coverage.coverage, LaneCoverage::NotConsidered { .. }))
            .map(|coverage| coverage.lane)
            .collect();
        let actual_lanes: BTreeSet<BlastLane> = lanes.iter().map(|verdict| verdict.lane).collect();
        mismatch(&expected_lanes, &actual_lanes, |lane| {
            lane.label().to_owned()
        })
        .map(|(missing, extra)| ApplyReason::LaneVerdictsMismatch { missing, extra })
        .map_or(Ok(()), Err)?;

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
        mismatch(&expected_symbols, &actual_symbols, |symbol| {
            (*symbol).to_owned()
        })
        .map(|(missing, extra)| ApplyReason::ConsumerChecksMismatch { missing, extra })
        .map_or(Ok(()), Err)?;

        let material: BTreeSet<&FindingId> = findings
            .iter()
            .filter(|finding| finding.is_material())
            .map(|finding| &finding.id)
            .collect();
        for verdict in lanes {
            if let LaneVerdictKind::Contested { finding } = &verdict.verdict
                && !material.contains(finding)
            {
                return Err(ApplyReason::ContestWithoutFinding { lane: verdict.lane });
            }
        }
        // §7: a consumer check that reports a symbol `Missing` must name a
        // material finding in this same review, exactly as a `Contested` lane
        // must. Without this the consumer-check obligation could be reported and
        // then defeated by a nonexistent or advisory finding.
        for check in consumers {
            if let ConsumerVerdict::Missing { finding } = &check.verdict
                && !material.contains(finding)
            {
                return Err(ApplyReason::MissingWithoutFinding {
                    symbol: check.symbol.clone(),
                });
            }
        }
    }
    Ok(())
}

/// The wave-close routing (design §8.2). Appends this wave's round to history
/// (empty triage and rulings), then routes on the material findings.
fn close_wave(
    candidate: &Candidate,
    wave: WaveNumber,
    reviews: OneOrMore<CompletedReview>,
    history: &[ReviewRound],
) -> RunState {
    let material: Vec<Finding> = wave_findings(&reviews)
        .filter(|finding| finding.is_material())
        .cloned()
        .collect();
    let reraises: Vec<FindingId> = wave_findings(&reviews)
        .filter(|finding| finding.reraises.is_some())
        .map(|finding| finding.id.clone())
        .collect();

    let mut history = history.to_vec();
    history.push(ReviewRound {
        wave,
        reviews: reviews.clone(),
        triage: Vec::new(),
        rulings: Vec::new(),
    });

    match OneOrMore::parse(material) {
        Err(_) => RunState::Final {
            candidate: candidate.clone(),
            history,
            accepted: Vec::new(),
            comments: Vec::new(),
        },
        Ok(unresolved) => {
            if !reraises.is_empty() {
                let findings = OneOrMore::parse(reraises)
                    .unwrap_or_else(|_| unresolved.map(|finding| finding.id.clone()));
                RunState::AwaitingReviewDecision {
                    candidate: candidate.clone(),
                    wave,
                    unresolved,
                    cause: ReviewEscalation::Reraised { findings },
                    history,
                }
            } else if wave.is_budget_exhausted() {
                RunState::AwaitingReviewDecision {
                    candidate: candidate.clone(),
                    wave,
                    unresolved,
                    cause: ReviewEscalation::BudgetExhausted,
                    history,
                }
            } else {
                RunState::Correcting {
                    candidate: candidate.clone(),
                    wave,
                    reviews,
                    must_fix: Vec::new(),
                    history,
                }
            }
        }
    }
}

fn next_from_correction(
    candidate: &Candidate,
    wave: WaveNumber,
    reviews: &OneOrMore<CompletedReview>,
    must_fix: &[FindingId],
    history: &[ReviewRound],
    result: CorrectionResult,
    repository: &Repository,
) -> Result<RunState, ApplyReason> {
    let corrected = Candidate::parse(*result.candidate, repository)?;

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
        return Err(ApplyReason::TriageMismatch { missing, extra });
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
            return Err(ApplyReason::MustFixRejected {
                finding: finding.clone(),
            });
        }
    }

    // 4. every Fix region actually changed.
    for triage in &result.triage {
        if let TriageOutcome::Fix { affected } = &triage.outcome {
            for region in affected {
                let before = candidate.fingerprints().get(region);
                let after = corrected.fingerprints().get(region);
                if before == after {
                    return Err(ApplyReason::FixTouchedNothing {
                        finding: triage.finding.clone(),
                        region: region.clone(),
                    });
                }
            }
        }
    }

    // 5. user-owned decisions unchanged.
    check_user_decisions_unchanged(candidate.raw(), corrected.raw())?;

    // 6. record the triage on this wave's round; enter the next wave.
    let history = record_triage(history, wave, &reviews.clone(), result.triage);
    let pending = derive_roster(&corrected);
    Ok(RunState::Reviewing {
        candidate: corrected,
        wave: wave.next(),
        pending,
        completed: Vec::new(),
        history,
    })
}

/// A user-owned decision may not be added, removed, or changed by a correction
/// (design decision 11). Planner decisions may.
fn check_user_decisions_unchanged(
    prior: &RawCandidate,
    corrected: &RawCandidate,
) -> Result<(), ApplyReason> {
    let prior_user: BTreeMap<&DecisionId, &Decision> = user_decisions(prior);
    let corrected_user: BTreeMap<&DecisionId, &Decision> = user_decisions(corrected);
    for (id, decision) in &prior_user {
        if corrected_user.get(id) != Some(decision) {
            return Err(ApplyReason::UserDecisionImmutable {
                decision: (*id).clone(),
            });
        }
    }
    for id in corrected_user.keys() {
        if !prior_user.contains_key(id) {
            return Err(ApplyReason::UserDecisionImmutable {
                decision: (*id).clone(),
            });
        }
    }
    Ok(())
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
    candidate: &Candidate,
    wave: WaveNumber,
    unresolved: &OneOrMore<Finding>,
    history: &[ReviewRound],
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

    let history = record_rulings(history, wave, result.rulings.clone());
    let must_fix: Vec<FindingId> = result
        .rulings
        .iter()
        .filter(|ruling| matches!(ruling.verdict, RulingVerdict::MustFix))
        .map(|ruling| ruling.finding.clone())
        .collect();

    if must_fix.is_empty() {
        let accepted: Vec<Finding> = unresolved
            .iter()
            .filter(|finding| {
                result.rulings.iter().any(|ruling| {
                    ruling.finding == finding.id && ruling.verdict == RulingVerdict::Accept
                })
            })
            .cloned()
            .collect();
        Ok(RunState::Final {
            candidate: candidate.clone(),
            history,
            accepted,
            comments: Vec::new(),
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
            candidate: candidate.clone(),
            wave,
            reviews,
            must_fix,
            history,
        })
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
    let document = RawPlanDocument {
        slug: plan.slug().clone(),
        repository: plan.repository().clone(),
        run: state.to_raw(),
    };
    let bytes = serde_json::to_vec(&document).unwrap_or_default();
    ParsedPlan::parse(&bytes)
}

fn mandate(kind: WorkKind) -> NonEmptyText {
    NonEmptyText::from_static(match kind {
        WorkKind::CapabilityReport => {
            "Report every available capability and every proposed addition with its cost and disclosed side effects."
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
    #[error("lane `{lane:?}` is contested without naming a material finding in this review")]
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
    #[error("user-owned decision `{decision}` was changed by a correction")]
    UserDecisionImmutable { decision: DecisionId },
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
    #[error("exploration result is not internally consistent: {detail}")]
    InvalidExploration { detail: String },
    #[error("candidate is not coherent: {detail}")]
    IncoherentCandidate { detail: String },
    #[error("internal transition error: {detail}")]
    Internal { detail: String },
}

impl From<CandidateError> for ApplyReason {
    fn from(error: CandidateError) -> Self {
        Self::IncoherentCandidate {
            detail: error.to_string(),
        }
    }
}

impl From<ExplorationContextError> for ApplyReason {
    fn from(error: ExplorationContextError) -> Self {
        Self::InvalidExploration {
            detail: error.to_string(),
        }
    }
}

impl From<ApprovedCapabilityError> for ApplyReason {
    fn from(error: ApprovedCapabilityError) -> Self {
        let ApprovedCapabilityError::UndisclosedProposal { proposal } = error;
        Self::UndisclosedProposal { proposal }
    }
}

/// A rejected transition, carrying the reason and the unchanged input plan.
#[derive(Debug)]
pub struct ApplyError {
    plan: Box<ParsedPlan>,
    reason: ApplyReason,
}

impl ApplyError {
    fn new(plan: ParsedPlan, reason: ApplyReason) -> Self {
        Self {
            plan: Box::new(plan),
            reason,
        }
    }

    pub const fn reason(&self) -> &ApplyReason {
        &self.reason
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
        write!(formatter, "{}", self.reason)
    }
}

impl std::error::Error for ApplyError {}
