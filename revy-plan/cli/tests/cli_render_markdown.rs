//! `render` writes Markdown mirroring the HTML projection (design §9).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;
use tempfile::TempDir;

const FINAL_PLAN: &str = include_str!("fixtures/final-plan.json");

fn render(body: &str) -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, body).expect("write plan");
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["render", "--out"])
        .arg(dir.path())
        .args(["--plan"])
        .arg(&plan)
        .assert()
        .success();
    let markdown = std::fs::read_to_string(dir.path().join("plan.md")).expect("plan.md written");
    (dir, markdown)
}

#[test]
fn render_writes_a_markdown_document_with_the_heading() {
    let (_dir, markdown) = render(FINAL_PLAN);
    // The brief carries a short title, so the H1 is the title and the summary
    // renders as the lead line below it.
    assert!(
        markdown.starts_with("# Bounded queue"),
        "{}",
        &markdown[..40.min(markdown.len())]
    );
    assert!(
        markdown.contains("Add a bounded queue"),
        "the summary is the lead"
    );
    assert!(markdown.contains("- In scope:"));
}

#[test]
fn render_markdown_carries_the_derived_figures() {
    let (_dir, markdown) = render(FINAL_PLAN);
    for heading in ["## Evidence map", "## Changed symbols", "## Delivery"] {
        assert!(markdown.contains(heading), "missing `{heading}`");
    }
    // The review timeline is rendered as a folded disclosure, not an H2.
    assert!(
        markdown.contains("<details><summary>Review timeline"),
        "missing folded review timeline"
    );
}

#[test]
fn render_does_not_mutate_the_plan_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, FINAL_PLAN).expect("write plan");
    let before = std::fs::read_to_string(&plan).expect("read plan");
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["render", "--out"])
        .arg(dir.path())
        .args(["--plan"])
        .arg(&plan)
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&plan).expect("read plan"),
        before,
        "render must not mutate"
    );
}
