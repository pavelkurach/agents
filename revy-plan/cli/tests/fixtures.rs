//! The on-disk plan fixtures the library's own unit tests embed via `include_str!`
//! / `include_bytes!`. This test proves each is admissible and regenerates it when
//! `REVY_WRITE_FIXTURES=1`, so a schema change is one command away from fresh
//! fixtures.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{final_accepted_document, final_plan_document, minimal_plan_document, parse};
use serde_json::Value;

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn regenerate(name: &str, document: &Value) {
    parse(document);
    if std::env::var_os("REVY_WRITE_FIXTURES").is_some() {
        let path = format!("{FIXTURE_DIR}/{name}");
        let pretty = serde_json::to_string_pretty(document).expect("serializes");
        std::fs::write(&path, format!("{pretty}\n")).expect("writes fixture");
    }
}

#[test]
fn minimal_plan_fixture_is_admissible() {
    regenerate("evidence-first-minimal-plan.json", &minimal_plan_document());
}

#[test]
fn final_plan_fixture_is_admissible() {
    regenerate("final-plan.json", &final_plan_document());
}

#[test]
fn final_accepted_plan_fixture_is_admissible() {
    regenerate("final-accepted-plan.json", &final_accepted_document());
}
