//! Release invariants (design §18): eight run states and ten work kinds, no
//! `OneOrMoreN` in the schema, no reviewer-authored resolution, and accepted
//! findings visible in both render formats.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;
use revy_plan::packet::{packet_kind_names, packet_schema_by_name, work_kind_schema};
use serde_json::Value;

const FINAL_ACCEPTED: &str = include_str!("fixtures/final-accepted-plan.json");

fn full_schema() -> Value {
    let assert = Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["schema"])
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).expect("schema is json")
}

#[test]
fn the_run_state_has_exactly_eight_variants() {
    let schema = full_schema();
    let variants = schema["$defs"]["RawRunState"]["oneOf"]
        .as_array()
        .expect("oneOf");
    assert_eq!(
        variants.len(),
        8,
        "RawRunState must have exactly eight variants"
    );
}

#[test]
fn the_work_kind_has_exactly_ten_kinds() {
    // Coupled to the enum, not a hand-list: the derived `WorkKind` schema carries
    // one `oneOf` entry per variant, so an eleventh variant fails this count.
    let schema = work_kind_schema();
    let variants = schema["oneOf"].as_array().expect("WorkKind oneOf");
    assert_eq!(variants.len(), 10, "WorkKind must have exactly ten kinds");

    // The names (derived from `WorkKind::ALL`) cover every variant, in order.
    let names = packet_kind_names();
    assert_eq!(
        names.len(),
        variants.len(),
        "packet_kind_names (from WorkKind::ALL) must cover every WorkKind variant"
    );
    assert_eq!(
        names,
        vec![
            "capability_report",
            "capability_approval",
            "exploration",
            "design_brief",
            "user_decision",
            "draft",
            "integrated_review",
            "specialist_review",
            "correction",
            "review_decision",
        ]
    );
    // Every kind resolves to a result schema.
    for name in names {
        assert!(
            packet_schema_by_name(name).is_some(),
            "`{name}` must have a result schema"
        );
    }
}

#[test]
fn the_full_schema_generates_no_one_or_more_definitions() {
    let schema = full_schema();
    let defs = schema["$defs"].as_object().expect("defs");
    let offending: Vec<&String> = defs
        .keys()
        .filter(|name| {
            name.strip_prefix("OneOrMore")
                .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
        })
        .collect();
    assert!(
        offending.is_empty(),
        "no OneOrMoreN defs may be generated: {offending:?}"
    );
}

#[test]
fn a_finding_carries_no_reviewer_authored_resolution() {
    let schema = full_schema();
    let properties = schema["$defs"]["Finding"]["properties"]
        .as_object()
        .expect("Finding props");
    assert!(
        !properties.contains_key("resolution"),
        "a reviewer may not resolve its own finding"
    );
}

#[test]
fn accepted_findings_are_visible_in_both_render_formats() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, FINAL_ACCEPTED).expect("write plan");
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["render", "--out"])
        .arg(dir.path())
        .args(["--plan"])
        .arg(&plan)
        .assert()
        .success();
    let markdown = std::fs::read_to_string(dir.path().join("plan.md")).expect("plan.md");
    let html = std::fs::read_to_string(dir.path().join("plan.html")).expect("plan.html");
    for format in [&markdown, &html] {
        assert!(
            format.contains("ACCEPTED WITH 1"),
            "the accepted banner must appear"
        );
        assert!(
            format.contains("deadlock"),
            "the accepted finding must be shown"
        );
    }
}
