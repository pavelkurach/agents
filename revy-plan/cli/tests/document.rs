//! The document load boundary (design §5, §6.6): JSON-pointer precision on the
//! adjacently-tagged run state, and the load proofs that re-prove the candidate
//! and that a final plan's accepted findings all appear in its history.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{clean_integrated_review, document, reviewing_document, simple_candidate, try_parse};
use serde_json::json;

#[test]
fn a_missing_top_level_variant_field_points_into_run_data() {
    let mut doc = reviewing_document(simple_candidate(), json!([{ "kind": "integrated" }]));
    doc["run"]["data"].as_object_mut().unwrap().remove("wave");
    let error = try_parse(&doc).expect_err("a missing wave is rejected");
    assert_eq!(error.pointer(), "/run/data/wave", "{error}");
    assert!(error.message().contains("missing field `wave`"));
}

#[test]
fn a_deep_missing_field_points_precisely_inside_the_candidate() {
    let mut doc = reviewing_document(simple_candidate(), json!([{ "kind": "integrated" }]));
    doc["run"]["data"]["candidate"]["brief"]
        .as_object_mut()
        .unwrap()
        .remove("summary");
    let error = try_parse(&doc).expect_err("a missing summary is rejected");
    assert_eq!(
        error.pointer(),
        "/run/data/candidate/brief/summary",
        "{error}"
    );
}

#[test]
fn an_unknown_field_inside_a_variant_is_located() {
    let mut doc = reviewing_document(simple_candidate(), json!([{ "kind": "integrated" }]));
    doc["run"]["data"]["candidate"]["brief"]["extra"] = json!("nope");
    let error = try_parse(&doc).expect_err("an unknown field is rejected");
    assert!(error.message().contains("unknown field `extra`"), "{error}");
}

#[test]
fn load_re_proves_the_candidate_in_a_final_state() {
    // Break the candidate's coverage; load must reject it even in `Final`.
    let mut candidate = simple_candidate();
    candidate["exploration"]["coverage"]
        .as_array_mut()
        .unwrap()
        .pop();
    let round = json!({
        "wave": 1, "reviews": [clean_integrated_review("tests")], "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": candidate, "history": [round], "accepted": [], "comments": [] }),
    );
    let error = try_parse(&doc).expect_err("an incoherent final candidate is rejected");
    assert!(error.to_string().contains("coverage checklist"), "{error}");
}

#[test]
fn an_accepted_finding_absent_from_history_is_rejected() {
    let phantom = json!({
        "id": "finding-phantom", "lens": "fact_check", "severity": "blocking",
        "statement": "Never reviewed.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 1, "reviews": [clean_integrated_review("tests")], "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": simple_candidate(), "history": [round],
                "accepted": [phantom], "comments": [] }),
    );
    let error = try_parse(&doc).expect_err("a fabricated accepted finding is rejected");
    assert!(
        error
            .to_string()
            .contains("does not appear in the review history"),
        "{error}"
    );
}

#[test]
fn a_final_plan_whose_accepted_finding_is_in_history_parses() {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "major",
        "statement": "Advisory but accepted.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": [{ "finding": "finding-1", "verdict": "accept" }]
    });
    let doc = document(
        "final",
        json!({ "candidate": simple_candidate(), "history": [round],
                "accepted": [finding], "comments": [] }),
    );
    try_parse(&doc).expect("an accepted finding present in history parses");
}
