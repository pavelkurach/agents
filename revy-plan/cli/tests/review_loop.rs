//! The review loop: findings, triage, rulings, and the reproductions of §1's
//! defects that the redesign makes impossible (design §8, §18.1).
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{
    StepError, apply_ok, blocking_integrated_review, candidate, clean_integrated_review,
    concurrency_candidate, confirmed_lane_verdicts, document, parse, reviewing_document,
    simple_candidate, step,
};
use revy_plan::packet::WorkKind;
use revy_plan::state::RunState;
use revy_plan::workflow::{ApplyReason, frontier};
use serde_json::json;

const fn integrated() -> WorkKind {
    WorkKind::IntegratedReview
}

const fn correction() -> WorkKind {
    WorkKind::Correction
}

const fn review_decision() -> WorkKind {
    WorkKind::ReviewDecision
}

// --- The four §1 reproductions -------------------------------------------------

#[test]
fn a_reviewer_authored_resolution_is_rejected_as_an_unknown_field() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated",
        "findings": [{
            "id": "finding-1", "lens": "fact_check", "severity": "blocking",
            "statement": "x", "regions": [{ "region": "brief" }],
            "resolution": { "state": "rejected" }
        }],
        "lanes": confirmed_lane_verdicts("tests"), "consumers": []
    });
    let error = step(plan, integrated(), &review).expect_err("a resolution field is rejected");
    let StepError::Decode(detail) = &error else {
        panic!("expected a decode rejection, got {error:?}");
    };
    assert!(
        detail.contains("resolution"),
        "the decode must reject the `resolution` field by name: {detail}"
    );
}

#[test]
fn a_finding_naming_a_region_the_candidate_lacks_is_rejected() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated",
        "findings": [{
            "id": "finding-1", "lens": "fact_check", "severity": "blocking",
            "statement": "dangling", "regions": [{ "region": "claim", "id": "claim-absent" }]
        }],
        "lanes": confirmed_lane_verdicts("tests"), "consumers": []
    });
    let error = step(plan, integrated(), &review).expect_err("a dangling region is rejected");
    assert!(
        matches!(error.reason(), ApplyReason::UnresolvedRegion { .. }),
        "{:?}",
        error.reason()
    );
}

#[test]
fn a_wave_with_a_material_finding_routes_to_correction_not_final() {
    // The self-cleared review is impossible: a material finding forces correction.
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let next = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    assert!(matches!(next.state(), RunState::Correcting { .. }));
}

#[test]
fn a_no_op_correction_that_touches_nothing_is_rejected() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    // The correction re-submits the byte-identical candidate and claims to fix the
    // brief — but the brief's fingerprint did not move.
    let result = json!({
        "candidate": simple_candidate(),
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "fix", "affected": [{ "region": "brief" }] } }]
    });
    let error =
        step(correcting, correction(), &result).expect_err("a no-op correction is rejected");
    assert!(
        matches!(error.reason(), ApplyReason::FixTouchedNothing { .. }),
        "{:?}",
        error.reason()
    );
}

#[test]
fn a_final_plan_carries_its_accepted_findings() {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "blocking",
        "statement": "The queue can deadlock.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": [{ "finding": "finding-1", "verdict": "accept" }]
    });
    let plan = parse(&document(
        "final",
        json!({ "candidate": simple_candidate(), "history": [round],
                "accepted": [finding], "comments": [] }),
    ));
    let RunState::Final { accepted, .. } = plan.state() else {
        panic!("expected a final plan")
    };
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].id.as_str(), "finding-1");
}

// --- Wave close routing --------------------------------------------------------

#[test]
fn a_clean_wave_clears_the_candidate_to_final() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let next = apply_ok(plan, integrated(), &clean_integrated_review("tests"));
    assert!(matches!(next.state(), RunState::Final { .. }));
}

#[test]
fn a_third_wave_with_material_findings_escalates_to_the_user() {
    let plan = parse(&document(
        "reviewing",
        json!({ "candidate": simple_candidate(), "wave": 3,
                "pending": [{ "kind": "integrated" }], "completed": [], "history": [] }),
    ));
    let next = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    assert!(matches!(
        next.state(),
        RunState::AwaitingReviewDecision { .. }
    ));
}

#[test]
fn a_reraised_rejected_finding_escalates_immediately() {
    let prior_round = json!({
        "wave": 1,
        "reviews": [{
            "kind": "integrated",
            "findings": [{ "id": "finding-old", "lens": "fact_check", "severity": "blocking",
                           "statement": "old", "regions": [{ "region": "brief" }] }],
            "lanes": [], "consumers": []
        }],
        "triage": [{ "finding": "finding-old", "outcome": { "kind": "reject", "rationale": "not real" } }],
        "rulings": []
    });
    let plan = parse(&document(
        "reviewing",
        json!({ "candidate": simple_candidate(), "wave": 2,
                "pending": [{ "kind": "integrated" }], "completed": [], "history": [prior_round] }),
    ));
    let review = json!({
        "kind": "integrated",
        "findings": [{
            "id": "finding-new", "lens": "fact_check", "severity": "blocking",
            "statement": "still broken", "regions": [{ "region": "brief" }],
            "reraises": "finding-old"
        }],
        "lanes": confirmed_lane_verdicts("tests"), "consumers": []
    });
    let next = apply_ok(plan, integrated(), &review);
    match next.state() {
        RunState::AwaitingReviewDecision { cause, .. } => {
            assert!(matches!(
                cause,
                revy_plan::model::review::ReviewEscalation::Reraised { .. }
            ));
        }
        other => panic!("expected escalation, got {other:?}"),
    }
}

#[test]
fn reraising_a_finding_no_triage_rejected_is_rejected() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated",
        "findings": [{
            "id": "finding-1", "lens": "fact_check", "severity": "blocking",
            "statement": "x", "regions": [{ "region": "brief" }], "reraises": "finding-unknown"
        }],
        "lanes": confirmed_lane_verdicts("tests"), "consumers": []
    });
    let error = step(plan, integrated(), &review).expect_err("an unknown reraise is rejected");
    assert!(matches!(error.reason(), ApplyReason::ReraiseUnknown { .. }));
}

// --- Correction ingest ---------------------------------------------------------

#[test]
fn a_triage_that_misses_a_material_finding_is_rejected() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    let mut corrected = simple_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue with backpressure");
    let result = json!({
        "candidate": corrected,
        "triage": [{ "finding": "finding-other", "outcome": { "kind": "reject", "rationale": "n/a" } }]
    });
    let error = step(correcting, correction(), &result).expect_err("triage mismatch is rejected");
    assert!(matches!(error.reason(), ApplyReason::TriageMismatch { .. }));
}

#[test]
fn a_correction_that_actually_changes_the_region_re_enters_review() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    let mut corrected = simple_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue with backpressure");
    let result = json!({
        "candidate": corrected,
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "fix", "affected": [{ "region": "brief" }] } }]
    });
    let next = apply_ok(correcting, correction(), &result);
    match next.state() {
        RunState::Reviewing { wave, .. } => assert_eq!(wave.get(), 2),
        other => panic!("expected wave 2 review, got {other:?}"),
    }
}

#[test]
fn a_correction_changing_a_user_decision_is_rejected() {
    let mut base = simple_candidate();
    let user_decision = json!({
        "id": "decision-user", "owner": "user", "question": "Which bound?",
        "state": { "state": "answered", "answer": "1024",
                   "consequences": ["Producers block at 1024."], "ruled_out": [] }
    });
    base["decisions"] = json!([user_decision]);
    let plan = parse(&reviewing_document(
        base.clone(),
        json!([{ "kind": "integrated" }]),
    ));
    let correcting = apply_ok(
        plan,
        integrated(),
        &blocking_integrated_review("tests", "finding-1"),
    );
    let mut corrected = base;
    // Change the brief (so the Fix genuinely touches it) AND the user decision
    // (which a correction may not do).
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");
    corrected["decisions"][0]["state"]["answer"] = json!("2048");
    let result = json!({
        "candidate": corrected,
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "fix", "affected": [{ "region": "brief" }] } }]
    });
    let error = step(correcting, correction(), &result).expect_err("a user decision is immutable");
    assert!(matches!(
        error.reason(),
        ApplyReason::UserDecisionImmutable { .. }
    ));
}

// --- Rulings -------------------------------------------------------------------

fn awaiting_decision_plan() -> revy_plan::document::ParsedPlan {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "blocking",
        "statement": "The queue can deadlock.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    parse(&document(
        "awaiting_review_decision",
        json!({ "candidate": simple_candidate(), "wave": 3, "unresolved": [finding],
                "cause": { "kind": "budget_exhausted" }, "history": [round] }),
    ))
}

#[test]
fn a_must_fix_ruling_returns_to_correction() {
    let plan = awaiting_decision_plan();
    let result = json!({ "rulings": [{ "finding": "finding-1", "verdict": "must_fix" }] });
    let next = apply_ok(plan, review_decision(), &result);
    match next.state() {
        RunState::Correcting { must_fix, .. } => {
            assert_eq!(must_fix.len(), 1);
        }
        other => panic!("expected correction, got {other:?}"),
    }
}

#[test]
fn a_dismiss_ruling_drops_the_finding_and_finalizes() {
    let plan = awaiting_decision_plan();
    let result = json!({ "rulings": [{ "finding": "finding-1", "verdict": "dismiss" }] });
    let next = apply_ok(plan, review_decision(), &result);
    match next.state() {
        RunState::Final { accepted, .. } => assert_eq!(accepted.as_slice(), []),
        other => panic!("expected final, got {other:?}"),
    }
}

#[test]
fn an_accept_ruling_finalizes_with_the_finding_recorded() {
    let plan = awaiting_decision_plan();
    let result = json!({ "rulings": [{ "finding": "finding-1", "verdict": "accept" }] });
    let next = apply_ok(plan, review_decision(), &result);
    match next.state() {
        RunState::Final { accepted, .. } => assert_eq!(accepted.len(), 1),
        other => panic!("expected final, got {other:?}"),
    }
}

#[test]
fn a_must_fix_finding_triaged_as_reject_is_rejected() {
    let plan = awaiting_decision_plan();
    let result = json!({ "rulings": [{ "finding": "finding-1", "verdict": "must_fix" }] });
    let correcting = apply_ok(plan, review_decision(), &result);
    let mut corrected = simple_candidate();
    corrected["brief"]["summary"] = json!("Add a bounded queue, revised");
    let correction_result = json!({
        "candidate": corrected,
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "reject", "rationale": "won't fix" } }]
    });
    let error = step(correcting, correction(), &correction_result)
        .expect_err("a must-fix finding cannot be rejected");
    assert!(matches!(
        error.reason(),
        ApplyReason::MustFixRejected { .. }
    ));
}

#[test]
fn rulings_must_cover_exactly_the_unresolved_findings() {
    let plan = awaiting_decision_plan();
    let result = json!({ "rulings": [{ "finding": "finding-other", "verdict": "dismiss" }] });
    let error = step(plan, review_decision(), &result).expect_err("rulings must match unresolved");
    assert!(matches!(
        error.reason(),
        ApplyReason::RulingsMismatch { .. }
    ));
}

// --- Parallel review keys and integrated obligations ---------------------------

#[test]
fn sibling_review_keys_stay_valid_after_one_apply() {
    let plan = parse(&reviewing_document(
        concurrency_candidate(),
        json!([{ "kind": "integrated" }, { "kind": "specialist", "focus": "concurrency" }]),
    ));
    let specialist_key = frontier(&plan)
        .iter()
        .find(|packet| matches!(packet.kind(), WorkKind::SpecialistReview { .. }))
        .map(|packet| packet.key().clone())
        .expect("a specialist packet is offered");

    let next = apply_ok(plan, integrated(), &clean_integrated_review("concurrency"));

    let still_offered = frontier(&next)
        .iter()
        .any(|packet| packet.key() == &specialist_key);
    assert!(
        still_offered,
        "the sibling specialist key must survive one apply"
    );
}

#[test]
fn the_specialist_packet_context_names_its_focus() {
    // The focus is carried in the packet context beside the other review fields,
    // so a driver reads which specialist to run from `status` rather than grepping
    // plan.json.
    let plan = parse(&reviewing_document(
        concurrency_candidate(),
        json!([{ "kind": "integrated" }, { "kind": "specialist", "focus": "concurrency" }]),
    ));
    let frontier = frontier(&plan);
    let specialist = frontier
        .iter()
        .find(|packet| matches!(packet.kind(), WorkKind::SpecialistReview { .. }))
        .expect("a specialist packet is offered");
    assert_eq!(
        specialist.context().get("focus"),
        Some(&json!("concurrency"))
    );
}

#[test]
fn lane_verdicts_must_cover_exactly_the_not_considered_lanes() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated", "findings": [],
        // Missing the other seven lanes.
        "lanes": [{ "lane": "api", "verdict": { "kind": "confirmed" } }],
        "consumers": []
    });
    let error = step(plan, integrated(), &review).expect_err("incomplete lanes rejected");
    assert!(matches!(
        error.reason(),
        ApplyReason::LaneVerdictsMismatch { .. }
    ));
}

#[test]
fn consumer_checks_must_cover_exactly_the_modified_symbols() {
    let plan = parse(&reviewing_document(
        candidate("tests", "modified"),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated", "findings": [],
        "lanes": confirmed_lane_verdicts("tests"),
        // The `push` symbol is modified, so it needs a consumer check.
        "consumers": []
    });
    let error = step(plan, integrated(), &review).expect_err("missing consumer check rejected");
    assert!(matches!(
        error.reason(),
        ApplyReason::ConsumerChecksMismatch { .. }
    ));
}

#[test]
fn a_contested_lane_without_a_material_finding_is_rejected() {
    let plan = parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
    let mut lanes = confirmed_lane_verdicts("tests");
    lanes[0] =
        json!({ "lane": "api", "verdict": { "kind": "contested", "finding": "finding-ghost" } });
    let review = json!({ "kind": "integrated", "findings": [], "lanes": lanes, "consumers": [] });
    let error = step(plan, integrated(), &review).expect_err("a ghost contest is rejected");
    assert!(matches!(
        error.reason(),
        ApplyReason::ContestWithoutFinding { .. }
    ));
}

#[test]
fn a_consumer_missing_naming_a_material_finding_is_accepted() {
    // §7: a `Missing` consumer verdict is admissible when it names a material
    // finding in this same review — the wave then routes to correction.
    let plan = parse(&reviewing_document(
        candidate("tests", "modified"),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated",
        "findings": [{
            "id": "finding-1", "lens": "consumers", "severity": "major",
            "statement": "A caller of `push` is unaccounted for.",
            "regions": [{ "region": "brief" }]
        }],
        "lanes": confirmed_lane_verdicts("tests"),
        "consumers": [{
            "symbol": "push", "method": "serena find_referencing_symbols",
            "references_found": 3, "verdict": { "kind": "missing", "finding": "finding-1" }
        }]
    });
    let next = apply_ok(plan, integrated(), &review);
    assert!(matches!(next.state(), RunState::Correcting { .. }));
}

#[test]
fn a_consumer_missing_without_a_material_finding_is_rejected() {
    // §7: a `Missing` consumer verdict that names no material finding (here a
    // nonexistent id) is rejected, exactly as a contested lane is.
    let plan = parse(&reviewing_document(
        candidate("tests", "modified"),
        json!([{ "kind": "integrated" }]),
    ));
    let review = json!({
        "kind": "integrated", "findings": [],
        "lanes": confirmed_lane_verdicts("tests"),
        "consumers": [{
            "symbol": "push", "method": "serena find_referencing_symbols",
            "references_found": 0, "verdict": { "kind": "missing", "finding": "finding-ghost" }
        }]
    });
    let error =
        step(plan, integrated(), &review).expect_err("a ghost consumer-missing is rejected");
    assert!(matches!(
        error.reason(),
        ApplyReason::MissingWithoutFinding { .. }
    ));
}
