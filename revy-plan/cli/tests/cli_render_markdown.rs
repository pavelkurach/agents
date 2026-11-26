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
const FINAL_ACCEPTED_PLAN: &str = include_str!("fixtures/final-accepted-plan.json");

fn render_with(body: &str, extra: &[&str]) -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = dir.path().join("plan.json");
    std::fs::write(&plan, body).expect("write plan");
    Command::cargo_bin("revy-plan")
        .expect("binary builds")
        .args(["render", "--out"])
        .arg(dir.path())
        .args(["--plan"])
        .arg(&plan)
        .args(extra)
        .assert()
        .success();
    let markdown = std::fs::read_to_string(dir.path().join("plan.md")).expect("plan.md written");
    (dir, markdown)
}

fn render(body: &str) -> (TempDir, String) {
    render_with(body, &[])
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
fn render_markdown_carries_the_derived_figures_under_one_trailing_appendix_heading() {
    let (_dir, markdown) = render(FINAL_PLAN);

    let (authored, appendix) = markdown
        .split_once("\n## Appendix\n")
        .expect("one Appendix heading");

    assert!(authored.contains("## Delivery"), "{markdown}");
    assert!(
        !appendix.contains("\n## "),
        "no H2 follows the appendix: {markdown}"
    );
    assert!(appendix.contains("### Evidence map"), "{markdown}");
    assert!(appendix.contains("### Impact areas"), "{markdown}");
    assert!(appendix.contains("### Changed symbols"), "{markdown}");
    assert!(
        appendix.contains("### Review timeline — 1 wave"),
        "{markdown}"
    );
}

#[test]
fn render_brief_omits_the_appendix_from_the_markdown() {
    let (_dir, markdown) = render_with(FINAL_PLAN, &["--brief"]);

    assert!(markdown.contains("## Delivery"), "{markdown}");
    assert!(!markdown.contains("Appendix"), "{markdown}");
    assert!(!markdown.contains("Evidence map"), "{markdown}");
    assert!(!markdown.contains("Review timeline"), "{markdown}");
}

#[test]
fn render_brief_keeps_the_accepted_banner_in_the_markdown() {
    let (_dir, markdown) = render_with(FINAL_ACCEPTED_PLAN, &["--brief"]);

    assert!(
        markdown.contains("> **ACCEPTED WITH 1 UNRESOLVED FINDINGS (1 blocking, 0 major)**"),
        "{markdown}"
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
