//! `status` emits the structured frontier object; `--schema` inlines each packet's
//! result schema, and no schema appears otherwise (design §7, §18.2).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;
use serde_json::{Value, json};

const FINAL_PLAN: &str = include_str!("fixtures/final-plan.json");

fn status(body: &str, extra: &[&str]) -> Value {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, body).expect("write plan");
    let mut args = vec!["status", "--plan"];
    args.push(plan.to_str().expect("utf8 path"));
    args.extend_from_slice(extra);
    let assert = Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(&args)
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).expect("status is json")
}

fn reviewing_from_final() -> String {
    let final_plan: Value = serde_json::from_str(FINAL_PLAN).expect("json");
    let doc = json!({
        "slug": final_plan["slug"], "repository": final_plan["repository"],
        "run": { "state": "reviewing", "data": {
            "candidate": final_plan["run"]["data"]["candidate"].clone(), "wave": 2,
            "pending": [{ "kind": "integrated" }], "completed": [], "history": []
        } }
    });
    serde_json::to_string(&doc).expect("json")
}

#[test]
fn status_of_a_final_plan_is_a_terminal_object() {
    let object = status(FINAL_PLAN, &[]);
    assert_eq!(object["state"], "final");
    assert_eq!(object["packets"].as_array().expect("packets").len(), 0);
    assert_eq!(object["accepted_findings"], json!(0));
}

#[test]
fn status_of_a_review_plan_names_the_wave_and_its_packet() {
    let object = status(&reviewing_from_final(), &[]);
    assert_eq!(object["state"], "reviewing");
    assert_eq!(object["wave"], json!(2));
    let packet = &object["packets"][0];
    for field in ["key", "kind", "subject", "mandate", "context", "schema_ref"] {
        assert!(
            packet.get(field).is_some(),
            "packet missing `{field}`: {packet}"
        );
    }
    assert_eq!(packet["schema_ref"], "integrated_review");
}

#[test]
fn status_emits_no_result_schema_without_the_flag() {
    let object = status(&reviewing_from_final(), &[]);
    assert!(
        object["packets"][0].get("schema").is_none(),
        "no schema without --schema"
    );
}

#[test]
fn status_inlines_result_schemas_with_the_flag() {
    let object = status(&reviewing_from_final(), &["--schema"]);
    assert!(
        object["packets"][0].get("schema").is_some(),
        "--schema inlines the result schema"
    );
}
