//! Workflow recovery and editing: reopening exploration or drafting with the
//! prior design carried, user/orchestrator revisions of a candidate under
//! review, the revision-relative wave budget, what a correction may do to a
//! user-owned decision, and the automatic capability approval.
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{
    apply_ok, blocking_integrated_review, clean_integrated_review, document, draft_result,
    drafting_document, final_plan_document, parse, report, simple_candidate, step, stores,
    unmoved_checkouts,
};
use revy_plan::document::ParsedPlan;
use revy_plan::ids::FindingId;
use revy_plan::model::content::DecisionState;
use revy_plan::packet::{RevisionResult, WorkKind};
use revy_plan::primitives::NonEmptyText;
use revy_plan::state::{ExplorationProgress, ReopenTarget, RunState};
use revy_plan::workflow::{ApplyReason, frontier, reopen, revise};
use serde_json::{Value, json};

fn reason(text: &str) -> NonEmptyText {
    NonEmptyText::parse(text).expect("non-empty")
}

/// The fixture candidate with one user decision answered `1024` over `4096`.
fn user_decision_candidate() -> Value {
    let mut candidate = simple_candidate();
    candidate["decisions"] = json!([{
        "id": "decision-bound", "owner": "user", "question": "Which bound?",
        "state": { "state": "answered", "answer": "1024",
                   "consequences": ["Producers block when full."],
                   "ruled_out": [{ "alternative": "4096", "reason": "not chosen by user" }] }
    }]);
    candidate
}

/// A plan reviewing `candidate` at wave 1, reached by drafting so it records
/// its approved capabilities.
fn reviewing(candidate: &Value) -> ParsedPlan {
    apply_ok(
        parse(&drafting_document(candidate)),
        WorkKind::Draft,
        &draft_result(candidate),
    )
}

/// A final plan over `candidate`, cleared by one clean wave.
fn final_plan(candidate: &Value) -> ParsedPlan {
    apply_ok(
        reviewing(candidate),
        WorkKind::IntegratedReview,
        &clean_integrated_review("tests"),
    )
}

fn complete_exploration(candidate: &Value) -> Value {
    let mut result = candidate["exploration"].clone();
    result["next"] = json!({ "kind": "complete" });
    result
}

fn brief_with_bound(alternatives: Value) -> Value {
    json!({
        "brief": { "title": "Bounded queue", "summary": "Add a bounded queue",
                   "scope": "the queue module", "out_of_scope": "everything else" },
        "decisions": [{ "owner": "user", "id": "decision-bound", "question": "Which bound?",
                        "alternatives": alternatives, "consequences": ["Producers block when full."] }]
    })
}

fn revise_with(
    plan: ParsedPlan,
    candidate: &Value,
    addresses: &[&str],
) -> Result<ParsedPlan, String> {
    let subject = json!({ "candidate": candidate });
    let addresses = addresses
        .iter()
        .map(|id| FindingId::parse(id).expect("finding id"))
        .collect();
    let revision = RevisionResult::decode(
        subject.to_string().as_bytes(),
        reason("Tighten the summary."),
        addresses,
    )
    .map_err(|error| error.to_string())?;
    let scratch = std::env::temp_dir();
    let checked =
        revy_plan::runtime::check_revision(&plan, stores(&scratch), &unmoved_checkouts(), revision)
            .map_err(|error| error.to_string())?;
    revise(plan, checked).map_err(|error| error.to_string())
}

fn revised_summary() -> Value {
    let mut candidate = simple_candidate();
    candidate["brief"]["summary"] = json!("Add a bounded queue with backpressure");
    candidate
}

// --- Reopen --------------------------------------------------------------------

#[test]
fn reopening_exploration_from_review_collects_over_the_candidate_exploration() {
    let plan = reviewing(&simple_candidate());

    let reopened = reopen(
        plan,
        ReopenTarget::Exploration,
        reason("Missed a consumer."),
    )
    .unwrap();

    let RunState::Exploring {
        capabilities,
        progress: ExplorationProgress::Collecting { prior },
        lineage,
    } = reopened.state()
    else {
        panic!(
            "expected collecting exploration, got {:?}",
            reopened.state()
        );
    };
    assert_eq!(capabilities.report().proposals.len(), 1);
    assert_eq!(prior.evidence().len(), 1);
    assert_eq!(lineage.reopens[0].reason.as_str(), "Missed a consumer.");
}

#[test]
fn reopening_a_commented_final_plan_keeps_its_comments_on_the_reopen() {
    let mut final_doc = final_plan_document();
    final_doc["run"]["data"]["comments"] =
        json!([{ "anchor": { "section": 0, "block": 0 }, "text": "Name the bound." }]);
    final_doc["run"]["data"]["lineage"] =
        json!({ "capabilities": { "report": report(), "approved": [] } });
    let plan = parse(&final_doc);

    let reopened = reopen(plan, ReopenTarget::Draft, reason("Split the MR.")).unwrap();

    assert_eq!(
        reopened.state().reopens()[0].comments[0].text.as_str(),
        "Name the bound."
    );
}

#[test]
fn reopening_exploration_keeps_the_set_aside_draft_on_the_prior_design() {
    let plan = reviewing(&simple_candidate());

    let reopened = reopen(plan, ReopenTarget::Exploration, reason("Recheck.")).unwrap();

    let RunState::Exploring { lineage, .. } = reopened.state() else {
        panic!("expected exploring");
    };
    let prior = lineage.prior_design.as_ref().expect("prior design");
    assert_eq!(prior.brief.summary.as_str(), "Add a bounded queue");
    assert_eq!(
        prior
            .draft
            .as_ref()
            .unwrap()
            .content
            .target_behavior
            .as_str(),
        "Work runs on a bounded queue."
    );
}

#[test]
fn reopening_exploration_before_capability_approval_is_unavailable() {
    let plan = parse(&common::minimal_plan_document());

    let error = reopen(plan, ReopenTarget::Exploration, reason("Too early.")).unwrap_err();

    assert_eq!(
        error.to_string(),
        "cannot reopen exploration from `awaiting_capability_approval`: capabilities are not approved yet; decide the capability report first"
    );
}

#[test]
fn reopening_a_draft_before_any_candidate_is_unavailable() {
    let plan = parse(&drafting_document(&simple_candidate()));

    let error = reopen(plan, ReopenTarget::Draft, reason("No draft yet.")).unwrap_err();

    assert_eq!(
        error.to_string(),
        "cannot reopen draft from `drafting`: no candidate exists yet; reopen draft needs reviewing, correcting, awaiting_review_decision, or final"
    );
}

#[test]
fn a_plan_whose_review_state_predates_recorded_capabilities_cannot_reopen() {
    let plan = parse(&final_plan_document());

    let error = reopen(plan, ReopenTarget::Draft, reason("Old plan.")).unwrap_err();

    assert!(matches!(
        error.reason(),
        ApplyReason::ReopenUnavailable { state: "final", .. }
    ));
}

#[test]
fn reopening_a_draft_returns_to_drafting_with_the_review_history_kept() {
    let plan = final_plan(&simple_candidate());

    let reopened = reopen(plan, ReopenTarget::Draft, reason("Split the MR.")).unwrap();

    let RunState::Drafting { lineage, .. } = reopened.state() else {
        panic!("expected drafting, got {:?}", reopened.state());
    };
    assert_eq!(lineage.reopens[0].history.len(), 1);
}

#[test]
fn a_redrafted_candidate_starts_review_at_wave_one_keeping_the_reopen() {
    let candidate = simple_candidate();
    let reopened = reopen(final_plan(&candidate), ReopenTarget::Draft, reason("Redo.")).unwrap();

    let reviewing = apply_ok(reopened, WorkKind::Draft, &draft_result(&candidate));

    assert_eq!(
        (
            reviewing.state().history().len(),
            reviewing.state().reopens().len()
        ),
        (0, 1)
    );
}

#[test]
fn an_unchanged_user_decision_keeps_its_prior_answer_after_reopening_exploration() {
    let candidate = user_decision_candidate();
    let reopened = reopen(
        reviewing(&candidate),
        ReopenTarget::Exploration,
        reason("More evidence."),
    )
    .unwrap();
    let briefing = apply_ok(
        reopened,
        WorkKind::Exploration,
        &complete_exploration(&candidate),
    );

    let drafting = apply_ok(
        briefing,
        WorkKind::DesignBrief,
        &brief_with_bound(json!(["4096", "1024"])),
    );

    let RunState::Drafting { decisions, .. } = drafting.state() else {
        panic!(
            "expected drafting without re-asking, got {:?}",
            drafting.state()
        );
    };
    let DecisionState::Answered { answer, .. } = &decisions[0].state else {
        panic!("expected an answered decision");
    };
    assert_eq!(answer.as_str(), "1024");
}

#[test]
fn a_user_decision_with_changed_alternatives_is_asked_again() {
    let candidate = user_decision_candidate();
    let reopened = reopen(
        reviewing(&candidate),
        ReopenTarget::Exploration,
        reason("More evidence."),
    )
    .unwrap();
    let briefing = apply_ok(
        reopened,
        WorkKind::Exploration,
        &complete_exploration(&candidate),
    );

    let asked = apply_ok(
        briefing,
        WorkKind::DesignBrief,
        &brief_with_bound(json!(["1024", "8192"])),
    );

    let RunState::AwaitingUserDecision { questions, .. } = asked.state() else {
        panic!("expected the question to be asked, got {:?}", asked.state());
    };
    assert_eq!(questions.first().decision.as_str(), "decision-bound");
}

/// A plan asked `decision-bound` with alternatives 1024 and 8192.
fn asked_bound() -> ParsedPlan {
    let candidate = user_decision_candidate();
    let reopened = reopen(
        reviewing(&candidate),
        ReopenTarget::Exploration,
        reason("More evidence."),
    )
    .unwrap();
    let briefing = apply_ok(
        reopened,
        WorkKind::Exploration,
        &complete_exploration(&candidate),
    );
    apply_ok(
        briefing,
        WorkKind::DesignBrief,
        &brief_with_bound(json!(["1024", "8192"])),
    )
}

fn answer_with_note() -> Value {
    json!({ "decision": "answer", "answers": [
        { "decision": "decision-bound", "chosen": "8192", "note": "Bursts reach 6k." }
    ] })
}

#[test]
fn a_user_answer_note_is_kept_on_the_answered_decision() {
    let drafting = apply_ok(asked_bound(), WorkKind::UserDecision, &answer_with_note());

    let RunState::Drafting { decisions, .. } = drafting.state() else {
        panic!("expected drafting, got {:?}", drafting.state());
    };
    assert_eq!(
        decisions[0].note.as_ref().map(NonEmptyText::as_str),
        Some("Bursts reach 6k.")
    );
}

#[test]
fn a_reopen_carries_the_answer_note_onto_the_prior_design() {
    let drafting = apply_ok(asked_bound(), WorkKind::UserDecision, &answer_with_note());

    let reopened = reopen(drafting, ReopenTarget::Exploration, reason("Again.")).unwrap();

    let RunState::Exploring { lineage, .. } = reopened.state() else {
        panic!("expected exploring");
    };
    assert_eq!(
        lineage.prior_design.as_ref().unwrap().answers[0]
            .note
            .as_ref()
            .map(NonEmptyText::as_str),
        Some("Bursts reach 6k.")
    );
}

#[test]
fn the_design_brief_packet_after_a_reopen_shows_the_prior_design() {
    let candidate = user_decision_candidate();
    let reopened = reopen(
        reviewing(&candidate),
        ReopenTarget::Exploration,
        reason("More evidence."),
    )
    .unwrap();
    let briefing = apply_ok(
        reopened,
        WorkKind::Exploration,
        &complete_exploration(&candidate),
    );

    let frontier = frontier(&briefing);
    let context = frontier.only().unwrap().context();

    assert_eq!(
        context["prior_design"]["answers"][0]["chosen"],
        json!("1024")
    );
}

// --- Revise --------------------------------------------------------------------

#[test]
fn revising_a_final_plan_enters_review_at_the_next_wave() {
    let plan = final_plan(&simple_candidate());

    let revised = revise_with(plan, &revised_summary(), &[]).unwrap();

    let RunState::Reviewing { wave, lineage, .. } = revised.state() else {
        panic!("expected reviewing, got {:?}", revised.state());
    };
    assert_eq!(
        (wave.get(), lineage.revisions[0].note.as_str()),
        (2, "Tighten the summary.")
    );
}

#[test]
fn a_revision_that_changes_nothing_is_rejected() {
    let plan = final_plan(&simple_candidate());

    let error = revise_with(plan, &simple_candidate(), &[]).unwrap_err();

    assert_eq!(
        error,
        "/candidate: the revised candidate is identical to the current one; a revision must change it"
    );
}

#[test]
fn a_revision_addressing_an_unknown_finding_is_rejected() {
    let plan = final_plan(&simple_candidate());

    let error = revise_with(plan, &revised_summary(), &["finding-nope"]).unwrap_err();

    assert_eq!(
        error,
        "/addresses/0: --addresses names finding `finding-nope`, which no review of this candidate raised (known: [])"
    );
}

#[test]
fn a_revision_may_address_a_finding_from_history() {
    let correcting = apply_ok(
        reviewing(&simple_candidate()),
        WorkKind::IntegratedReview,
        &blocking_integrated_review("tests", "finding-1"),
    );

    let revised = revise_with(correcting, &revised_summary(), &["finding-1"]).unwrap();

    assert_eq!(
        revised.state().revisions()[0].addresses[0].as_str(),
        "finding-1"
    );
}

#[test]
fn revising_before_a_candidate_exists_is_unavailable() {
    let plan = parse(&drafting_document(&simple_candidate()));
    let subject = json!({ "candidate": simple_candidate() });
    let revision = RevisionResult::decode(
        subject.to_string().as_bytes(),
        reason("Too early."),
        Vec::new(),
    )
    .unwrap();
    let scratch = std::env::temp_dir();
    let checked =
        revy_plan::runtime::check_revision(&plan, stores(&scratch), &unmoved_checkouts(), revision)
            .unwrap();

    let error = revise(plan, checked).unwrap_err();

    assert_eq!(
        error.to_string(),
        "revise applies to reviewing, correcting, awaiting_review_decision, or final; the plan is `drafting`"
    );
}

fn wave_three_finding() -> Value {
    json!({
        "id": "finding-1", "lens": "fact_check", "severity": "blocking",
        "statement": "The queue can deadlock.", "regions": [{ "region": "brief" }]
    })
}

/// A plan escalated to the user at wave 3 over one blocking finding.
fn escalated() -> ParsedPlan {
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [wave_three_finding()], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    parse(&document(
        "awaiting_review_decision",
        json!({ "candidate": simple_candidate(), "wave": 3, "unresolved": [wave_three_finding()],
                "cause": { "kind": "budget_exhausted" }, "history": [round] }),
    ))
}

/// The escalated plan after the user ruled its finding must-fix.
fn correcting_must_fix() -> ParsedPlan {
    apply_ok(
        escalated(),
        WorkKind::ReviewDecision,
        &json!({ "rulings": [{ "finding": "finding-1", "verdict": "must_fix" }] }),
    )
}

#[test]
fn the_wave_budget_counts_waves_since_the_latest_revision() {
    let revised = revise_with(correcting_must_fix(), &revised_summary(), &["finding-1"]).unwrap();

    let next = apply_ok(
        revised,
        WorkKind::IntegratedReview,
        &blocking_integrated_review("tests", "finding-2"),
    );

    assert!(
        matches!(next.state(), RunState::Correcting { wave, .. } if wave.get() == 4),
        "{:?}",
        next.state()
    );
}

#[test]
fn revising_a_correction_without_addressing_its_must_fix_findings_is_rejected() {
    let error = revise_with(correcting_must_fix(), &revised_summary(), &[]).unwrap_err();

    assert_eq!(
        error,
        "/addresses: a revision of a `correcting` plan must address every finding the user ruled must-fix; add --addresses finding-1"
    );
}

#[test]
fn revising_an_escalation_without_addressing_every_unresolved_finding_is_rejected() {
    let error = revise_with(escalated(), &revised_summary(), &[]).unwrap_err();

    assert_eq!(
        error,
        "/addresses: a revision of an `awaiting_review_decision` plan must address every unresolved finding; add --addresses finding-1"
    );
}

#[test]
fn a_revision_of_an_escalation_records_the_escalation_cause() {
    let revised = revise_with(escalated(), &revised_summary(), &["finding-1"]).unwrap();

    assert_eq!(
        serde_json::to_value(&revised.state().revisions()[0].cause).unwrap(),
        json!({ "kind": "budget_exhausted" })
    );
}

#[test]
fn a_revision_of_an_escalation_does_not_buy_fresh_automatic_waves() {
    let revised = revise_with(escalated(), &revised_summary(), &["finding-1"]).unwrap();

    let next = apply_ok(
        revised,
        WorkKind::IntegratedReview,
        &blocking_integrated_review("tests", "finding-2"),
    );

    assert!(
        matches!(next.state(), RunState::AwaitingReviewDecision { wave, .. } if wave.get() == 4),
        "{:?}",
        next.state()
    );
}

fn accepted_final() -> ParsedPlan {
    parse(&common::final_accepted_document())
}

#[test]
fn an_accepted_finding_reappears_at_the_next_final_after_a_revision() {
    let revised = revise_with(accepted_final(), &revised_summary(), &[]).unwrap();

    let next = apply_ok(
        revised,
        WorkKind::IntegratedReview,
        &clean_integrated_review("tests"),
    );

    let accepted: Vec<&str> = next
        .state()
        .accepted()
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(accepted, ["finding-1"]);
}

#[test]
fn an_accepted_finding_a_revision_addresses_does_not_reappear() {
    let revised = revise_with(accepted_final(), &revised_summary(), &["finding-1"]).unwrap();

    let next = apply_ok(
        revised,
        WorkKind::IntegratedReview,
        &clean_integrated_review("tests"),
    );

    assert_eq!(next.state().accepted(), []);
}

#[test]
fn an_accepted_finding_survives_a_second_unrelated_revision() {
    let once = revise_with(accepted_final(), &revised_summary(), &[]).unwrap();
    let mut twice_candidate = revised_summary();
    twice_candidate["brief"]["title"] = json!("Bounded queue, again");
    let twice = revise_with(once, &twice_candidate, &[]).unwrap();

    let next = apply_ok(
        twice,
        WorkKind::IntegratedReview,
        &clean_integrated_review("tests"),
    );

    let accepted: Vec<&str> = next
        .state()
        .accepted()
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(accepted, ["finding-1"]);
}

// --- Corrections and user decisions -------------------------------------------

fn correcting_user_decision_plan() -> ParsedPlan {
    apply_ok(
        reviewing(&user_decision_candidate()),
        WorkKind::IntegratedReview,
        &blocking_integrated_review("tests", "finding-1"),
    )
}

fn brief_fix(candidate: Value) -> Value {
    json!({
        "candidate": candidate,
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "fix", "affected": [{ "region": "brief" }] } }]
    })
}

#[test]
fn a_correction_may_reword_a_user_decision_question() {
    let mut corrected = user_decision_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");
    corrected["decisions"][0]["question"] = json!("Which queue bound?");

    let next = apply_ok(
        correcting_user_decision_plan(),
        WorkKind::Correction,
        &brief_fix(corrected),
    );

    assert!(matches!(next.state(), RunState::Reviewing { .. }));
}

#[test]
fn a_correction_changing_a_user_decision_answer_names_the_answer() {
    let mut corrected = user_decision_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");
    corrected["decisions"][0]["state"]["answer"] = json!("4096");
    corrected["decisions"][0]["state"]["ruled_out"] =
        json!([{ "alternative": "1024", "reason": "not chosen by user" }]);

    let error = step(
        correcting_user_decision_plan(),
        WorkKind::Correction,
        &brief_fix(corrected),
    )
    .unwrap_err();

    assert_eq!(
        error.reason().to_string(),
        "user-owned decision `decision-bound` had its `answer` changed by a correction; a correction may reword a user decision's question, consequences, or ruled-out reasons, never its answer or alternatives — those change only with the user's approval: `revise` records the change for the user to see"
    );
}

#[test]
fn a_correction_changing_a_user_decision_alternatives_is_rejected() {
    let mut corrected = user_decision_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");
    corrected["decisions"][0]["state"]["ruled_out"] =
        json!([{ "alternative": "8192", "reason": "not chosen by user" }]);

    let error = step(
        correcting_user_decision_plan(),
        WorkKind::Correction,
        &brief_fix(corrected),
    )
    .unwrap_err();

    assert!(matches!(
        error.reason(),
        ApplyReason::UserDecisionImmutable {
            change: revy_plan::workflow::DecisionChange::Alternatives,
            ..
        }
    ));
}

#[test]
fn a_revision_may_change_a_user_decision_answer() {
    let mut revised = user_decision_candidate();
    revised["decisions"][0]["state"]["answer"] = json!("4096");
    revised["decisions"][0]["state"]["ruled_out"] =
        json!([{ "alternative": "1024", "reason": "not chosen by user" }]);

    let next = revise_with(final_plan(&user_decision_candidate()), &revised, &[]).unwrap();

    assert!(matches!(next.state(), RunState::Reviewing { .. }));
}

#[test]
fn a_revision_changing_a_user_decision_answer_records_the_decision_change() {
    let mut revised = user_decision_candidate();
    revised["decisions"][0]["state"]["answer"] = json!("4096");
    revised["decisions"][0]["state"]["ruled_out"] =
        json!([{ "alternative": "1024", "reason": "not chosen by user" }]);

    let next = revise_with(final_plan(&user_decision_candidate()), &revised, &[]).unwrap();

    assert_eq!(
        serde_json::to_value(&next.state().revisions()[0].decision_changes).unwrap(),
        json!([{ "decision": "decision-bound", "field": "answer", "before": "1024", "after": "4096" }])
    );
}

#[test]
fn a_revision_changing_a_user_decision_alternatives_records_both_sets() {
    let mut revised = user_decision_candidate();
    revised["decisions"][0]["state"]["ruled_out"] =
        json!([{ "alternative": "8192", "reason": "not chosen by user" }]);

    let next = revise_with(final_plan(&user_decision_candidate()), &revised, &[]).unwrap();

    assert_eq!(
        serde_json::to_value(&next.state().revisions()[0].decision_changes).unwrap(),
        json!([{ "decision": "decision-bound", "field": "alternatives",
                 "before": ["1024", "4096"], "after": ["1024", "8192"] }])
    );
}

#[test]
fn a_revision_leaving_user_decisions_alone_records_no_decision_change() {
    let next = revise_with(final_plan(&simple_candidate()), &revised_summary(), &[]).unwrap();

    assert_eq!(next.state().revisions()[0].decision_changes, []);
}

fn with_unapproved_tool_failure(candidate: Value) -> Value {
    let mut candidate = candidate;
    candidate["exploration"]["tool_failures"] =
        json!([{ "proposal": "proposal-serena", "reason": "It crashed." }]);
    candidate
}

#[test]
fn a_correction_recording_a_failure_of_an_unapproved_tool_is_rejected() {
    let mut corrected = with_unapproved_tool_failure(user_decision_candidate());
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");

    let error = step(
        correcting_user_decision_plan(),
        WorkKind::Correction,
        &brief_fix(corrected),
    )
    .unwrap_err();

    assert_eq!(
        error.reason().to_string(),
        "exploration result is not internally consistent: tool failure names proposal `proposal-serena`, which is not an approved proposal (approved: none)"
    );
}

#[test]
fn a_revision_recording_a_failure_of_an_unapproved_tool_is_rejected() {
    let error = revise_with(
        final_plan(&simple_candidate()),
        &with_unapproved_tool_failure(revised_summary()),
        &[],
    )
    .unwrap_err();

    assert_eq!(
        error,
        "/candidate/exploration/tool_failures/0/proposal: exploration result is not internally consistent: tool failure names proposal `proposal-serena`, which is not an approved proposal (approved: none)"
    );
}

fn reopened_draft_context() -> Value {
    let reopened = reopen(
        reviewing(&simple_candidate()),
        ReopenTarget::Draft,
        reason("Split the MR."),
    )
    .unwrap();
    frontier(&reopened).only().unwrap().context().clone()
}

#[test]
fn the_draft_packet_after_a_reopen_lists_the_prior_drafts_addressable_ids() {
    let context = reopened_draft_context();

    assert_eq!(
        context["prior_draft"]["ids"],
        json!({
            "requirements": [], "acceptance_criteria": ["ac-1"], "risks": [], "mrs": ["mr-1"],
            "changed_symbols": ["push"], "sections": [{ "index": 0, "heading": "Overview" }]
        })
    );
}

#[test]
fn the_draft_packet_after_a_reopen_documents_the_patch_shape() {
    let context = reopened_draft_context();

    assert_eq!(
        context["prior_draft"]["patch"]["example"][0]["path"],
        json!("/delivery/mrs/@mr-1/title")
    );
}

#[test]
fn the_correction_packet_context_documents_the_patch_shape() {
    let plan = correcting_user_decision_plan();

    let frontier = frontier(&plan);
    let context = frontier.only().unwrap().context();

    assert_eq!(
        context["patch"]["example"][0]["path"],
        json!("/candidate/content/risks/@risk-cache/statement")
    );
}

// --- Capability approval and report stamping ----------------------------------

fn semantic_report(proposals: Value) -> Value {
    json!({
        "capabilities": [{ "id": "capability-lsp", "name": "rust-analyzer", "version": null,
                           "interface": "lsp", "proves": ["semantic"], "limitations": [] }],
        "proposals": proposals
    })
}

#[test]
fn a_revised_report_with_nothing_to_approve_still_waits_for_the_user() {
    let plan = parse(&common::minimal_plan_document());

    let next = apply_ok(
        plan,
        WorkKind::CapabilityReport,
        &semantic_report(json!([])),
    );

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

fn revision_requested() -> ParsedPlan {
    apply_ok(
        parse(&common::minimal_plan_document()),
        WorkKind::CapabilityApproval,
        &json!({ "decision": "request_revision", "report": common::report_digest(),
                 "instruction": "Drop serena; use rust-analyzer." }),
    )
}

#[test]
fn a_report_answering_a_revision_request_waits_for_the_user_even_with_nothing_to_approve() {
    let plan = revision_requested();

    let next = apply_ok(
        plan,
        WorkKind::CapabilityReport,
        &semantic_report(json!([])),
    );

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn the_capability_report_packet_shows_the_requested_revision() {
    let plan = revision_requested();

    let frontier = frontier(&plan);
    let packet = frontier
        .iter()
        .find(|packet| packet.kind() == WorkKind::CapabilityReport)
        .unwrap();

    assert_eq!(
        packet.context()["revision_request"],
        json!("Drop serena; use rust-analyzer.")
    );
}

#[test]
fn a_report_after_a_mid_exploration_proposal_waits_for_the_user_even_with_nothing_to_approve() {
    let exploring = parse(&document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    ));
    let proposed = apply_ok(
        exploring,
        WorkKind::Exploration,
        &json!({
            "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": [],
            "next": { "kind": "propose_capability", "report": semantic_report(report()["proposals"].clone()) }
        }),
    );

    let next = apply_ok(
        proposed,
        WorkKind::CapabilityReport,
        &semantic_report(json!([])),
    );

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn a_revised_report_with_a_proposal_still_waits_for_the_user() {
    let plan = parse(&common::minimal_plan_document());

    let next = apply_ok(
        plan,
        WorkKind::CapabilityReport,
        &semantic_report(report()["proposals"].clone()),
    );

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn a_revised_report_without_code_intelligence_still_waits_for_the_user() {
    let plan = parse(&common::minimal_plan_document());
    let mut textual = semantic_report(json!([]));
    textual["capabilities"][0]["proves"] = json!(["textual"]);

    let next = apply_ok(plan, WorkKind::CapabilityReport, &textual);

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn a_revised_report_without_a_repository_is_stamped_with_the_plan_identity() {
    let plan = parse(&common::minimal_plan_document());

    let next = apply_ok(
        plan,
        WorkKind::CapabilityReport,
        &report_without_repository(),
    );

    let RunState::AwaitingCapabilityApproval { report, .. } = next.state() else {
        panic!("expected awaiting approval");
    };
    assert_eq!(report.repository.revision.as_str(), common::REVISION);
}

fn report_without_repository() -> Value {
    let mut value = report();
    value.as_object_mut().unwrap().remove("repository");
    value
}

#[test]
fn a_mid_exploration_proposal_always_waits_even_with_nothing_to_approve() {
    let plan = parse(&document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [] },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    ));
    let result = json!({
        "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": [],
        "next": { "kind": "propose_capability", "report": semantic_report(json!([])) }
    });

    let next = apply_ok(plan, WorkKind::Exploration, &result);

    assert!(matches!(
        next.state(),
        RunState::AwaitingCapabilityApproval { .. }
    ));
}

#[test]
fn an_automatic_approval_over_a_report_with_proposals_is_rejected_at_load() {
    let caps_doc = document(
        "exploring",
        json!({
            "capabilities": { "report": report(), "approved": [], "approval": "auto" },
            "progress": { "phase": "collecting", "prior": {
                "evidence": [], "claims": [], "uncertainties": [], "impact": [], "coverage": []
            } }
        }),
    );

    let error = common::try_parse(&caps_doc).unwrap_err();

    assert!(
        error
            .message()
            .contains("automatic approval requires a report with no proposals"),
        "{error}"
    );
}

#[test]
fn a_capability_decision_without_a_digest_binds_to_the_current_report() {
    let plan = parse(&common::minimal_plan_document());

    let next = apply_ok(
        plan,
        WorkKind::CapabilityApproval,
        &json!({ "decision": "proceed_current" }),
    );

    assert!(matches!(next.state(), RunState::Exploring { .. }));
}

#[test]
fn a_capability_decision_with_a_stale_digest_is_still_rejected() {
    let plan = parse(&common::minimal_plan_document());

    let error = step(
        plan,
        WorkKind::CapabilityApproval,
        &json!({ "decision": "proceed_current", "report": common::DIGEST }),
    )
    .unwrap_err();

    assert_eq!(
        error.reason().to_string(),
        "capability decision digest does not match the report"
    );
}

#[test]
fn a_reopened_exploration_previews_the_set_aside_candidate() {
    let candidate = user_decision_candidate();
    let reopened = reopen(
        reviewing(&candidate),
        ReopenTarget::Exploration,
        reason("More evidence."),
    )
    .unwrap();

    let preview = revy_plan::workflow::reopened_preview(reopened.state()).unwrap();

    assert_eq!(serde_json::to_value(preview.raw()).unwrap(), candidate);
}
