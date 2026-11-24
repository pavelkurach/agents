//! Reviewer-roster derivation from a candidate's impact shape and authored risks
//! (design §8.1). Each row is driven through the draft→review transition, which is
//! where the roster is computed and stored as `pending`.
#![allow(
    clippy::indexing_slicing,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use common::{apply_ok, candidate, draft_result, drafting_document, parse, simple_candidate};
use revy_plan::model::content::SpecialistFocus;
use revy_plan::model::review::ReviewKind;
use revy_plan::packet::WorkKind;
use revy_plan::state::RunState;
use serde_json::{Value, json};

/// The specialist focuses the derived roster requires for `candidate`.
fn specialists(candidate: &Value) -> Vec<SpecialistFocus> {
    let plan = parse(&drafting_document(candidate));
    let next = apply_ok(plan, WorkKind::Draft, &draft_result(candidate));
    match next.state() {
        RunState::Reviewing { pending, .. } => pending
            .iter()
            .filter_map(|kind| match kind {
                ReviewKind::Specialist { focus } => Some(*focus),
                ReviewKind::Integrated => None,
            })
            .collect(),
        other => panic!("expected review, got {other:?}"),
    }
}

/// `covered_lane` covers a behavioral `queue` change; assert the required focus.
fn assert_lane_selects(covered_lane: &str, focus: SpecialistFocus) {
    let selected = specialists(&candidate(covered_lane, "added"));
    assert!(
        selected.contains(&focus),
        "{covered_lane} must select {focus:?}: got {selected:?}"
    );
}

#[test]
fn the_integrated_review_is_always_required() {
    let plan = parse(&drafting_document(&simple_candidate()));
    let next = apply_ok(plan, WorkKind::Draft, &draft_result(&simple_candidate()));
    match next.state() {
        RunState::Reviewing { pending, .. } => {
            assert!(
                pending
                    .iter()
                    .any(|kind| matches!(kind, ReviewKind::Integrated))
            );
        }
        other => panic!("expected review, got {other:?}"),
    }
}

#[test]
fn concurrency_lane_behavioral_selects_concurrency() {
    assert_lane_selects("concurrency", SpecialistFocus::Concurrency);
}

#[test]
fn data_at_rest_lane_affected_selects_migration() {
    assert_lane_selects("data_at_rest", SpecialistFocus::Migration);
}

#[test]
fn security_lane_affected_selects_security() {
    assert_lane_selects("security", SpecialistFocus::Security);
}

#[test]
fn compatibility_lane_behavioral_selects_protocol_compatibility() {
    assert_lane_selects("compatibility", SpecialistFocus::ProtocolCompatibility);
}

#[test]
fn resource_bounds_lane_affected_selects_performance() {
    assert_lane_selects("resource_bounds", SpecialistFocus::Performance);
}

#[test]
fn api_lane_behavioral_selects_public_api() {
    assert_lane_selects("api", SpecialistFocus::PublicApi);
}

#[test]
fn observability_configuration_and_tests_select_no_specialist() {
    for lane in ["observability", "configuration", "tests"] {
        assert!(
            specialists(&candidate(lane, "added")).is_empty(),
            "lane {lane} must select no specialist"
        );
    }
}

#[test]
fn a_mechanical_change_under_concurrency_selects_no_concurrency_specialist() {
    // Concurrency requires a *behavioral* change; a mechanical one does not.
    let mut candidate = candidate("concurrency", "added");
    candidate["exploration"]["impact"][0]["entry"]["shape"] = json!("mechanical");
    assert!(!specialists(&candidate).contains(&SpecialistFocus::Concurrency));
}

#[test]
fn an_authored_specialist_risk_joins_the_roster() {
    let mut candidate = simple_candidate();
    candidate["content"]["risks"] = json!([{
        "kind": "specialist", "id": "risk-1", "focus": "security",
        "statement": "Adds a new trust boundary.", "mitigation": "Threat-model it."
    }]);
    assert!(specialists(&candidate).contains(&SpecialistFocus::Security));
}
