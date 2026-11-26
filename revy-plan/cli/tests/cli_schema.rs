//! `schema` prints the full plan schema, one named section, or a packet's result
//! schema; the full schema inlines `OneOrMore` (design §10, §18.2).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;
use serde_json::Value;

fn schema_output(args: &[&str]) -> Value {
    let assert = Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(args)
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).expect("schema is json")
}

#[test]
fn the_schema_is_derived_from_the_raw_plan_document() {
    let schema = schema_output(&["schema"]);
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "RawPlanDocument");
    assert!(
        schema["$defs"].get("RawRunState").is_some(),
        "missing RawRunState"
    );
}

#[test]
fn the_schema_generates_no_one_or_more_definitions() {
    let schema = schema_output(&["schema"]);
    let defs = schema["$defs"].as_object().expect("defs object");
    let offending: Vec<&String> = defs
        .keys()
        .filter(|name| {
            name.strip_prefix("OneOrMore")
                .is_some_and(|rest| rest.chars().next().is_some_and(|c| c.is_ascii_digit()))
        })
        .collect();
    assert!(
        offending.is_empty(),
        "OneOrMoreN defs must not be generated: {offending:?}"
    );
}

#[test]
fn a_named_section_prints_one_definition() {
    let section = schema_output(&["schema", "RawRunState"]);
    assert!(
        section.get("oneOf").is_some(),
        "not the RawRunState schema: {section}"
    );
}

#[test]
fn an_unknown_section_lists_available_names() {
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["schema", "Nonsense"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("RawRunState"));
}

#[test]
fn a_packet_result_schema_prints_for_a_known_kind() {
    // Every packet kind's result schema is available by name.
    for kind in [
        "integrated_review",
        "correction",
        "draft",
        "capability_approval",
    ] {
        let schema = schema_output(&["schema", "--packet", kind]);
        assert!(schema.is_object(), "packet `{kind}` must have a schema");
    }
}

#[test]
fn an_unknown_packet_kind_is_a_usage_error() {
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["schema", "--packet", "nonsense"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unknown packet kind"));
}

#[test]
fn an_id_in_a_packet_schema_publishes_the_slug_pattern() {
    let schema = schema_output(&["schema", "--packet", "exploration"]);

    assert_eq!(
        schema["$defs"]["Claim"]["properties"]["id"]["pattern"],
        "^[a-z0-9]+(-[a-z0-9]+)*$"
    );
}

#[test]
fn an_anchor_path_in_a_packet_schema_publishes_the_repository_qualified_path_pattern() {
    let schema = schema_output(&["schema", "--packet", "exploration"]);

    assert_eq!(
        schema["$defs"]["Location"]["properties"]["path"]["pattern"],
        revy_plan::primitives::REPO_PATH_PATTERN
    );
}
