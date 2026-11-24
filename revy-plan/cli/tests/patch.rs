//! The RFC 6902 patch subset over a packet's subject document (design §10):
//! add/replace/remove, array `-` append, a bad pointer, and re-parsing the patched
//! document into the result type.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::simple_candidate;
use revy_plan::packet::CorrectionResult;
use revy_plan::patch::{PatchOperation, apply_all};
use serde_json::{Value, json};

fn ops(value: Value) -> Vec<PatchOperation> {
    serde_json::from_value(value).expect("valid ops")
}

fn patched(document: &mut Value, operations: Value) {
    apply_all(document, &ops(operations)).expect("patch applies");
}

#[test]
fn add_replace_and_remove_edit_the_document() {
    let mut document = json!({ "a": 1, "b": [10] });
    patched(
        &mut document,
        json!([
            { "op": "add", "path": "/c", "value": 3 },
            { "op": "replace", "path": "/a", "value": 9 },
            { "op": "remove", "path": "/b/0" }
        ]),
    );
    assert_eq!(document, json!({ "a": 9, "b": [], "c": 3 }));
}

#[test]
fn a_dash_appends_to_an_array() {
    let mut document = json!({ "triage": [] });
    patched(
        &mut document,
        json!([
            { "op": "add", "path": "/triage/-",
              "value": { "finding": "finding-1", "outcome": { "kind": "reject", "rationale": "n/a" } } }
        ]),
    );
    assert_eq!(document["triage"].as_array().unwrap().len(), 1);
}

#[test]
fn a_bad_pointer_reports_the_failing_op() {
    let mut document = json!({ "a": 1 });
    let error = apply_all(
        &mut document,
        &ops(json!([
            { "op": "replace", "path": "/does/not/exist", "value": 1 }
        ])),
    )
    .expect_err("a bad pointer fails");
    assert_eq!(error.index, 0);
    assert_eq!(error.op, "replace");
}

#[test]
fn a_patched_correction_subject_re_parses_as_the_result_type() {
    // The correction subject document is `{ candidate, triage: [] }`.
    let mut document = json!({ "candidate": simple_candidate(), "triage": [] });
    patched(
        &mut document,
        json!([
            { "op": "replace", "path": "/candidate/brief/summary", "value": "Add a bounded queue, revised" },
            { "op": "add", "path": "/triage/-",
              "value": { "finding": "finding-1", "outcome": { "kind": "fix",
                         "affected": [{ "region": "brief" }] } } }
        ]),
    );
    let result: CorrectionResult = serde_json::from_value(document).expect("re-parses");
    assert_eq!(
        result.candidate.brief.summary.as_str(),
        "Add a bounded queue, revised"
    );
    assert_eq!(result.triage.len(), 1);
}

#[test]
fn a_patch_that_breaks_the_shape_fails_to_re_parse() {
    let mut document = json!({ "candidate": simple_candidate(), "triage": [] });
    patched(
        &mut document,
        json!([
            { "op": "remove", "path": "/candidate/brief/summary" }
        ]),
    );
    serde_json::from_value::<CorrectionResult>(document)
        .expect_err("a candidate missing its summary is not a valid result");
}
