//! Rendering the plan (design §9): the draft watermark, the accepted banner, and
//! the four mandatory derived figures — the evidence → claim map with a sampled
//! marker, the changed-symbol tree, the as-is call graph, and the review timeline.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{
    DIGEST, clean_integrated_review, document, render_document, reviewing_document,
    simple_candidate,
};
use serde_json::{Value, json};

/// A candidate carrying call-edges evidence, a sampled textual evidence, and
/// several changed symbols across directories — enough to exercise every figure.
fn rich_candidate() -> Value {
    let mut candidate = simple_candidate();
    let evidence = candidate["exploration"]["evidence"].as_array_mut().unwrap();
    evidence.push(json!({
        "kind": "semantic", "id": "evidence-calls", "provenance": { "tool": "serena", "version": null },
        "operation": "find_referencing_symbols", "subject": "push",
        "observation": { "shape": "call_edges", "edges": [
            { "caller": { "path": "src/api.rs", "line": 10, "column": 1 },
              "callee": { "path": "src/queue.rs", "line": 5, "column": 1 } }
        ], "completeness": { "kind": "complete" } },
        "target": { "revision": "rev1", "dirty_digest": DIGEST }, "limitation": null
    }));
    evidence.push(json!({
        "kind": "textual", "id": "evidence-grep", "scope": ["src/queue.rs"], "pattern": "push",
        "matches": [{ "location": { "path": "src/queue.rs", "line": 5, "column": 1 }, "line": "fn push" }],
        "completeness": { "kind": "sampled", "total": 40 },
        "target": { "revision": "rev1", "dirty_digest": DIGEST }, "limitation": null
    }));
    candidate["exploration"]["claims"][0]["evidence"] = json!(["evidence-queue", "evidence-grep"]);
    candidate["content"]["changed_symbols"] = json!([
        { "symbol": "push", "path": "src/queue.rs", "change": "added" },
        { "symbol": "pop", "path": "src/queue.rs", "change": "modified" },
        { "symbol": "main", "path": "src/bin/main.rs", "change": "added" }
    ]);
    candidate["delivery"]["mrs"][0]["files"] = json!(["src/queue.rs", "src/bin/main.rs"]);
    candidate
}

fn reviewing() -> Value {
    reviewing_document(rich_candidate(), json!([{ "kind": "integrated" }]))
}

#[test]
fn a_non_final_plan_carries_a_draft_watermark() {
    let rendered = render_document(&reviewing());
    assert!(
        rendered.markdown.contains("DRAFT — reviewing"),
        "markdown watermark"
    );
    assert!(
        rendered.html.contains("DRAFT — reviewing"),
        "html watermark"
    );
}

#[test]
fn a_final_plan_with_accepted_findings_renders_the_banner_first() {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "blocking",
        "statement": "The queue can deadlock under contention.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 3,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": [{ "finding": "finding-1", "verdict": "accept" }]
    });
    let doc = document(
        "final",
        json!({ "candidate": rich_candidate(), "history": [round],
                "accepted": [finding], "comments": [] }),
    );
    let rendered = render_document(&doc);
    assert!(
        rendered
            .markdown
            .contains("ACCEPTED WITH 1 UNRESOLVED FINDINGS (1 blocking, 0 major)"),
        "markdown banner"
    );
    assert!(rendered.html.contains("ACCEPTED WITH 1"), "html banner");
    // The banner precedes the orientation/scope narrative.
    let banner = rendered.html.find("ACCEPTED WITH").unwrap();
    let scope = rendered.html.find("In scope").unwrap();
    assert!(
        banner < scope,
        "the accepted banner must come before the plan body"
    );
}

#[test]
fn the_evidence_map_marks_sampled_evidence() {
    let rendered = render_document(&reviewing());
    assert!(rendered.markdown.contains("Evidence map"));
    assert!(rendered.html.contains("Evidence map"));
    // `evidence-grep` was a sample of 40; the map marks it.
    assert!(
        rendered.markdown.contains("sampled"),
        "markdown sampled marker"
    );
    assert!(rendered.html.contains("sampled"), "html sampled marker");
}

#[test]
fn the_changed_symbol_tree_lists_every_symbol_with_its_change_kind() {
    let rendered = render_document(&reviewing());
    assert!(rendered.markdown.contains("Changed symbols"));
    for symbol in ["push", "pop", "main"] {
        assert!(rendered.html.contains(symbol), "tree must name {symbol}");
    }
    assert!(
        rendered.html.contains("tre-modified"),
        "the modified symbol is coloured"
    );
}

#[test]
fn the_as_is_call_graph_shows_the_call_edges() {
    let rendered = render_document(&reviewing());
    assert!(rendered.markdown.contains("As-is call graph"));
    assert!(rendered.html.contains("As-is call graph"));
    assert!(
        rendered.markdown.contains("src/api.rs:10"),
        "the caller anchor"
    );
    assert!(
        rendered.markdown.contains("src/queue.rs:5"),
        "the callee anchor"
    );
}

#[test]
fn the_review_timeline_shows_each_wave() {
    let finding = json!({
        "id": "finding-1", "lens": "fact_check", "severity": "major",
        "statement": "The bound is unstated.", "regions": [{ "region": "brief" }]
    });
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated", "findings": [finding], "lanes": [], "consumers": [] }],
        "triage": [{ "finding": "finding-1", "outcome": { "kind": "reject", "rationale": "the bound is in the AC" } }],
        "rulings": []
    });
    let clean = json!({
        "wave": 2, "reviews": [clean_integrated_review("tests")], "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": rich_candidate(), "history": [round, clean],
                "accepted": [], "comments": [] }),
    );
    let rendered = render_document(&doc);
    assert!(rendered.markdown.contains("Review timeline"));
    assert!(rendered.markdown.contains("Wave 1"));
    assert!(rendered.markdown.contains("Wave 2"));
    assert!(
        rendered
            .markdown
            .contains("rejected: the bound is in the AC")
    );
}

#[test]
fn the_verify_command_appears_in_the_acceptance_criteria() {
    let rendered = render_document(&reviewing());
    assert!(
        rendered.markdown.contains("manual: Fill the queue"),
        "the AC verify is shown"
    );
}

#[test]
fn autocode_wraps_bare_code_shapes_in_rendered_prose() {
    let mut candidate = rich_candidate();
    candidate["exploration"]["claims"][0]["statement"] =
        json!("The run_render arm skips println! and writes plan.md");
    let rendered = render_document(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ));
    // HTML: the high-confidence shapes become <code> spans.
    for needle in [
        "<code>run_render</code>",
        "<code>println!</code>",
        "<code>plan.md</code>",
    ] {
        assert!(rendered.html.contains(needle), "missing {needle}");
    }
    // Markdown mirror carries the same tokens as backtick spans.
    for needle in ["`run_render`", "`println!`", "`plan.md`"] {
        assert!(
            rendered.markdown.contains(needle),
            "markdown missing {needle}: {}",
            rendered.markdown
        );
    }
}

#[test]
fn the_acceptance_criteria_lead_with_the_statement_and_demote_the_slug() {
    let rendered = render_document(&reviewing());
    // Item 6: the criteria render as a lead-with-statement list, not a slug-first
    // table; the statement is the bold headline and the slug drops to a caption.
    assert!(
        rendered.html.contains("lead-list"),
        "acceptance criteria render as a lead-with-statement list"
    );
    assert!(
        rendered
            .markdown
            .contains("- **AC: Push blocks when the queue is full.**"),
        "markdown AC headline: {}",
        rendered.markdown
    );
    assert!(
        rendered.markdown.contains("ac-1 · verify:"),
        "the slug sits in the caption, not the headline"
    );
}
