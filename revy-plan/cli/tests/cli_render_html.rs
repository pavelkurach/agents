//! `render` writes standalone HTML for any candidate-bearing state; a non-final
//! plan carries the draft watermark (design §9, §11).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use assert_cmd::Command;
use serde_json::{Value, json};
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
    let html = std::fs::read_to_string(dir.path().join("plan.html")).expect("plan.html written");
    (dir, html)
}

fn render(body: &str) -> (TempDir, String) {
    render_with(body, &[])
}

/// The page past its inlined stylesheet, whose comments name sections literally.
fn page_body(html: &str) -> &str {
    html.split_once("</style>").expect("inlined style").1
}

fn reviewing_from_final() -> String {
    let final_plan: Value = serde_json::from_str(FINAL_PLAN).expect("json");
    let candidate = final_plan["run"]["data"]["candidate"].clone();
    let doc = json!({
        "slug": final_plan["slug"], "repository": final_plan["repository"],
        "run": { "state": "reviewing", "data": {
            "candidate": candidate, "wave": 1,
            "pending": [{ "kind": "integrated" }], "completed": [], "history": []
        } }
    });
    serde_json::to_string(&doc).expect("json")
}

#[test]
fn render_writes_a_standalone_html_document() {
    let (_dir, html) = render(FINAL_PLAN);
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(
        html.contains("<style>"),
        "styles are inlined for standalone use"
    );
    assert!(html.contains("Add a bounded queue"));
}

#[test]
fn render_html_carries_the_derived_figures_and_not_the_retired_sections() {
    let (_dir, html) = render(FINAL_PLAN);
    for heading in [
        "Evidence map",
        "Impact areas",
        "Changed symbols",
        "Review timeline",
        "Delivery",
    ] {
        assert!(html.contains(heading), "missing `{heading}`");
    }
    // The section headings are gone (the words survive only in inlined CSS comments).
    for retired in [
        "<h2>Verification</h2>",
        "Risk-selected reviews</h2>",
        "<h2>Advisory findings</h2>",
    ] {
        assert!(
            !html.contains(retired),
            "the retired heading `{retired}` must be gone"
        );
    }
}

#[test]
fn render_watermarks_a_non_final_plan() {
    let (_dir, html) = render(&reviewing_from_final());
    assert!(
        html.contains("DRAFT — reviewing"),
        "a non-final render carries a watermark"
    );
}

#[test]
fn render_html_folds_the_derived_figures_into_one_collapsed_appendix() {
    let (_dir, html) = render(FINAL_PLAN);

    let (authored, appendix) = page_body(&html)
        .split_once("<details class=\"section secondary appendix\">")
        .expect("one collapsed appendix disclosure");

    assert_eq!(page_body(&html).matches("<details").count(), 1, "{html}");
    assert!(authored.contains("<h2>Delivery</h2>"), "{html}");
    assert!(!authored.contains("Evidence map"), "{html}");
    assert!(appendix.contains("<h3>Evidence map</h3>"), "{html}");
    assert!(appendix.contains("<h3>Review timeline</h3>"), "{html}");
}

#[test]
fn render_brief_omits_the_appendix_from_the_html() {
    let (_dir, html) = render_with(FINAL_PLAN, &["--brief"]);

    assert!(page_body(&html).contains("<h2>Delivery</h2>"), "{html}");
    assert!(!page_body(&html).contains("Appendix"), "{html}");
    assert!(!page_body(&html).contains("Evidence map"), "{html}");
    assert!(!page_body(&html).contains("Review timeline"), "{html}");
}

#[test]
fn render_brief_keeps_the_accepted_banner_in_the_html() {
    let (_dir, html) = render_with(FINAL_ACCEPTED_PLAN, &["--brief"]);

    assert!(
        html.contains("ACCEPTED WITH 1 UNRESOLVED FINDINGS (1 blocking, 0 major)"),
        "{html}"
    );
}

#[test]
fn render_brief_keeps_the_draft_watermark() {
    let (_dir, html) = render_with(&reviewing_from_final(), &["--brief"]);

    assert!(html.contains("DRAFT — reviewing"), "{html}");
}
