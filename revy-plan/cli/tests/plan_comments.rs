//! The review-comment overlay is stored on the final state under `run.data`,
//! outside the candidate, so leaving a comment never changes the candidate
//! fingerprint that keys the review packets.
#![allow(
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use revy_plan::document::ParsedPlan;
use revy_plan::model::discussion::BlockAnchor;
use revy_plan::state::{RunState, candidate_fingerprint};
use serde_json::{Value, json};

const FINAL_PLAN: &[u8] = include_bytes!("fixtures/final-plan.json");

fn fingerprint(bytes: &[u8]) -> String {
    let plan = ParsedPlan::parse(bytes).expect("the fixture parses");
    let candidate = plan
        .state()
        .candidate()
        .expect("a final plan carries a candidate");
    candidate_fingerprint(candidate).as_str().to_owned()
}

#[test]
fn a_final_plan_with_no_comments_has_an_empty_overlay() {
    let plan = ParsedPlan::parse(FINAL_PLAN).expect("the fixture parses");
    assert_eq!(plan.state().comments(), []);
    assert!(matches!(plan.state(), RunState::Final { .. }));
}

#[test]
fn adding_a_comment_does_not_change_the_candidate_fingerprint() {
    let before = fingerprint(FINAL_PLAN);

    let mut document: Value = serde_json::from_slice(FINAL_PLAN).expect("the fixture is JSON");
    document["run"]["data"]["comments"] = json!([
        { "anchor": { "section": 0, "block": 0 }, "text": "this path double-locks", "resolved": false }
    ]);
    let with_comment = serde_json::to_vec(&document).expect("re-serializes");

    let plan = ParsedPlan::parse(&with_comment).expect("parses with a comment");
    assert_eq!(plan.state().comments().len(), 1);
    assert_eq!(plan.state().comments()[0].anchor, BlockAnchor::new(0, 0));

    assert_eq!(
        before,
        fingerprint(&with_comment),
        "a comment must not touch the candidate"
    );
}
