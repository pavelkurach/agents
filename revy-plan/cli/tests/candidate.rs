//! Candidate coherence proofs at load (design §6.6): delivery graph, coverage
//! totality, evidence grounding, changed-symbol containment, and uncertainty
//! disposition resolution.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{parse, reviewing_document, simple_candidate, try_parse};
use serde_json::{Value, json};

/// Assert a candidate mutation is rejected at load with a message containing
/// `needle`.
fn reject(mutate: impl FnOnce(&mut Value), needle: &str) {
    let mut candidate = simple_candidate();
    mutate(&mut candidate);
    let document = reviewing_document(candidate, json!([{ "kind": "integrated" }]));
    let error = try_parse(&document).expect_err("the defect must be rejected");
    assert!(
        error.message().contains(needle) || error.to_string().contains(needle),
        "expected `{needle}`, got: {error}"
    );
}

#[test]
fn a_coherent_candidate_parses() {
    parse(&reviewing_document(
        simple_candidate(),
        json!([{ "kind": "integrated" }]),
    ));
}

#[test]
fn an_acceptance_criterion_no_merge_request_owns_is_rejected() {
    reject(
        |candidate| {
            candidate["delivery"]["acceptance_criteria"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "id": "ac-orphan", "statement": "Nobody owns this.",
                    "verify": { "kind": "manual", "procedure": "n/a" }
                }));
        },
        "not owned by exactly one",
    );
}

#[test]
fn a_missing_blast_lane_is_rejected() {
    reject(
        |candidate| {
            candidate["exploration"]["coverage"]
                .as_array_mut()
                .unwrap()
                .pop();
        },
        "missing from the coverage checklist",
    );
}

#[test]
fn a_covered_lane_naming_an_undeclared_subject_is_rejected() {
    reject(
        |candidate| {
            // The `tests` lane covers `queue`; point it at a phantom subject.
            for entry in candidate["exploration"]["coverage"].as_array_mut().unwrap() {
                if entry["lane"] == json!("tests") {
                    entry["coverage"]["subjects"] = json!(["phantom"]);
                }
            }
        },
        "not a declared impact subject",
    );
}

#[test]
fn a_covered_lane_naming_only_not_applicable_subjects_is_rejected() {
    reject(
        |candidate| {
            candidate["exploration"]["impact"][0]["entry"] =
                json!({ "state": "not_applicable", "rationale": "No queue here." });
        },
        "not-applicable",
    );
}

#[test]
fn evidence_observed_against_another_repository_is_rejected() {
    reject(
        |candidate| {
            candidate["exploration"]["evidence"][0]["target"]["revision"] = json!("other-rev");
        },
        "STALE-MODEL",
    );
}

#[test]
fn a_changed_symbol_outside_every_merge_request_file_is_rejected() {
    reject(
        |candidate| {
            candidate["content"]["changed_symbols"][0]["path"] = json!("src/unlisted.rs");
        },
        "no merge request lists",
    );
}

#[test]
fn a_duplicate_changed_symbol_is_rejected() {
    reject(
        |candidate| {
            candidate["content"]["changed_symbols"]
                .as_array_mut()
                .unwrap()
                .push(json!({ "symbol": "push", "path": "src/queue.rs", "change": "modified" }));
        },
        "duplicate changed symbol",
    );
}

#[test]
fn an_uncertainty_accepted_by_a_non_user_decision_is_rejected() {
    reject(
        |candidate| {
            candidate["exploration"]["uncertainties"] = json!([{
                "id": "uncertainty-1", "question": "Which bound?",
                "disposition": { "state": "accepted_by_user", "decision": "decision-missing" }
            }]);
        },
        "does not resolve to a user-owned answered decision",
    );
}

#[test]
fn an_uncertainty_contained_by_an_unknown_criterion_is_rejected() {
    reject(
        |candidate| {
            candidate["exploration"]["uncertainties"] = json!([{
                "id": "uncertainty-1", "question": "Does it hold under load?",
                "disposition": { "state": "contained_by_criterion", "criterion": "ac-missing" }
            }]);
        },
        "criterion `ac-missing` does not resolve",
    );
}

#[test]
fn an_uncertainty_contained_by_a_real_criterion_parses() {
    let mut candidate = simple_candidate();
    candidate["exploration"]["uncertainties"] = json!([{
        "id": "uncertainty-1", "question": "Does it hold under load?",
        "disposition": { "state": "contained_by_criterion", "criterion": "ac-1" }
    }]);
    parse(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ));
}
