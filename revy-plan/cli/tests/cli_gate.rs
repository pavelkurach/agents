//! `gate` succeeds only once the plan is final, and refuses a final plan carrying
//! accepted unresolved findings unless `--allow-accepted` (design §11, §18.1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;

const FINAL_PLAN: &str = include_str!("fixtures/final-plan.json");
const FINAL_ACCEPTED: &str = include_str!("fixtures/final-accepted-plan.json");
const MINIMAL_PLAN: &str = include_str!("fixtures/evidence-first-minimal-plan.json");

fn revy_plan() -> Command {
    Command::cargo_bin("revy-plan").expect("binary `revy-plan` builds")
}

fn write(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("plan.json");
    std::fs::write(&path, body).expect("write plan");
    path
}

#[test]
fn gate_succeeds_for_a_clean_final_plan() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = write(dir.path(), FINAL_PLAN);
    revy_plan()
        .args(["gate", "--plan"])
        .arg(&plan)
        .assert()
        .success();
}

#[test]
fn gate_fails_before_the_plan_is_final() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = write(dir.path(), MINIMAL_PLAN);
    revy_plan()
        .args(["gate", "--plan"])
        .arg(&plan)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("not reached its final state"));
}

#[test]
fn gate_fails_on_accepted_findings_without_the_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = write(dir.path(), FINAL_ACCEPTED);
    revy_plan()
        .args(["gate", "--plan"])
        .arg(&plan)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("accepted unresolved finding"));
}

#[test]
fn gate_passes_accepted_findings_with_the_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = write(dir.path(), FINAL_ACCEPTED);
    revy_plan()
        .args(["gate", "--allow-accepted", "--plan"])
        .arg(&plan)
        .assert()
        .success();
}

#[test]
fn gate_reports_a_malformed_plan_as_a_usage_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = write(dir.path(), "{ not json");
    revy_plan()
        .args(["gate", "--plan"])
        .arg(&plan)
        .assert()
        .code(2);
}
