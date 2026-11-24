//! Visual-density lock: a plan spanning the full authored and derived surface
//! renders every rich element, so a future renderer change can only add richness,
//! never silently drop a figure (design §9). Assertion-based rather than a
//! byte-exact golden, so it does not break on incidental markup churn.
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{DIGEST, document, final_plan_document, render_document, simple_candidate};
use serde_json::{Value, json};

/// A candidate exercising orientation, a code excerpt, an authored figure, call
/// edges, a sampled evidence, several changed symbols, decisions, and a risk.
fn dense_candidate() -> Value {
    let mut candidate = simple_candidate();
    candidate["decisions"] = json!([{
        "id": "decision-bound", "owner": "planner", "question": "Which bound?",
        "state": { "state": "answered", "answer": "1024",
                   "consequences": ["Producers block at 1024."],
                   "ruled_out": [{ "alternative": "unbounded", "reason": "Memory blows up." }] }
    }]);
    candidate["content"]["risks"] = json!([{
        "kind": "ordinary", "id": "risk-1", "statement": "Lock contention.",
        "mitigation": "Shard the queue."
    }]);
    candidate["content"]["orientation"] = json!({
        "placement": "Between the handler and the pool.", "touched_flows": ["submit"],
        "glossary": [{ "term": "worker", "definition": "Drains the queue." }]
    });
    candidate["content"]["sections"] = json!([{
        "heading": "Design",
        "blocks": [
            { "kind": "prose", "text": "The queue is **bounded**." },
            { "kind": "code", "excerpt": { "lang": "rust", "caption": null, "code": "fn push() {}" } },
            { "kind": "figure", "diagram": { "kind": "states", "states": [
                { "name": "Idle", "kind": null, "note": null }], "transitions": [] } }
        ]
    }]);
    let evidence = candidate["exploration"]["evidence"].as_array_mut().unwrap();
    evidence.push(json!({
        "kind": "semantic", "id": "evidence-calls", "provenance": { "tool": "serena", "version": null },
        "operation": "find_referencing_symbols", "subject": "push",
        "observation": { "shape": "call_edges", "edges": [
            { "caller": { "path": "src/api.rs", "line": 10, "column": 1 },
              "callee": { "path": "src/queue.rs", "line": 5, "column": 1 } }
        ], "completeness": { "kind": "sampled", "total": 12 } },
        "target": { "revision": "rev1", "dirty_digest": DIGEST }, "limitation": null
    }));
    candidate["exploration"]["claims"][0]["evidence"] = json!(["evidence-queue", "evidence-calls"]);
    candidate["content"]["changed_symbols"] = json!([
        { "symbol": "push", "path": "src/queue.rs", "change": "added" },
        { "symbol": "drain", "path": "src/worker/pool.rs", "change": "removed" }
    ]);
    candidate["delivery"]["mrs"][0]["files"] = json!(["src/queue.rs", "src/worker/pool.rs"]);
    candidate
}

fn dense_final() -> Value {
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated",
            "findings": [{ "id": "finding-1", "lens": "evidence", "severity": "advisory",
                           "statement": "Consider a metric.", "regions": [{ "region": "brief" }] }],
            "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    document(
        "final",
        json!({ "candidate": dense_candidate(), "history": [round],
                "accepted": [], "comments": [] }),
    )
}

#[test]
fn the_dense_plan_renders_every_rich_element() {
    let rendered = render_document(&dense_final());
    let html = &rendered.html;
    let needed = [
        ("orientation", "Orientation"),
        ("code figure", "figure class=\"code\""),
        ("authored figure", "dg-figure"),
        ("evidence map", "Evidence map"),
        ("sampled marker", "sampled"),
        ("changed-symbol tree", "d tree"),
        ("removed symbol colour", "tre-removed"),
        ("call graph", "As-is call graph"),
        ("review timeline", "Review timeline"),
        ("coverage strip", "coverage"),
        ("impact table", "Impact areas"),
        ("decisions", "Decisions"),
        ("ruled out", "Considered and rejected"),
        ("delivery", "Delivery"),
        ("mr dag", "mr-dag"),
        ("authored behavior figure", "dg-figure behavior-figure"),
        ("lead-with-statement list", "lead-list"),
        ("demoted slug caption", "item-cap"),
        ("folded timeline", "timeline-fold"),
    ];
    for (name, needle) in needed {
        assert!(
            html.contains(needle),
            "the {name} section (`{needle}`) must render"
        );
    }
    // A final plan carries no draft watermark.
    assert!(
        !html.contains("DRAFT —"),
        "a final plan has no draft watermark"
    );
}

#[test]
fn an_authored_dag_figure_suppresses_the_derived_mr_dag() {
    // Baseline: with no authored dag, the derived MR-DAG renders exactly once.
    let baseline = render_document(&dense_final());
    assert_eq!(
        baseline.html.matches("mr-dag").count(),
        1,
        "the derived MR-DAG renders once when no dag is authored"
    );

    // Author a `dag` figure: the renderer must suppress the derived MR-DAG so the
    // page carries one dependency graph, never two (design §9 de-dup).
    let mut candidate = dense_candidate();
    candidate["content"]["sections"][0]["blocks"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "kind": "figure",
            "diagram": {
                "kind": "dag", "caption": null, "edges": [],
                "waves": [{
                    "name": null, "gate": null,
                    "mrs": [{ "id": "mr-1", "title": "Add the bounded queue",
                              "kind": null, "flag": null, "run": null, "note": null }]
                }]
            }
        }));
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated", "findings": [], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": candidate, "history": [round], "accepted": [], "comments": [] }),
    );
    let rendered = render_document(&doc);
    assert_eq!(
        rendered.html.matches("mr-dag").count(),
        0,
        "an authored dag figure suppresses the derived MR-DAG (no double MR-DAG)"
    );
    assert!(
        rendered.html.contains("d dag"),
        "the authored dag figure still renders"
    );
}

#[test]
fn a_behavior_figure_heads_the_area_with_the_prose_never_folded() {
    // dense_candidate carries an authored states figure: it heads the behavior
    // area, and the current/target prose stays fully visible full-width directly
    // below it — behavior is never wrapped in a <details> fold.
    let with_figure = render_document(&dense_final());
    assert!(
        with_figure.html.contains("dg-figure behavior-figure"),
        "an authored figure heads the behavior area"
    );
    assert!(
        with_figure.html.contains("class=\"prose behavior-prose\""),
        "the behavior prose stays full-width body text below the figure"
    );
    assert!(with_figure.html.contains("Current behavior."));
    // Never folded — no behavior <details> in either format.
    assert!(
        !with_figure.html.contains("behavior-detail")
            && !with_figure.html.contains("Behavior in words"),
        "behavior is never wrapped in a fold: {}",
        with_figure.html
    );
    assert!(
        !with_figure.markdown.contains("Behavior in words"),
        "md behavior is never folded: {}",
        with_figure.markdown
    );
    // The behavior prose renders before any <details> element in the body, so it
    // can never sit inside the timeline/reference folds either. (Scope past the
    // inlined CSS, whose comments mention <details> literally.)
    let body = with_figure
        .html
        .split("</style>")
        .nth(1)
        .expect("the page has a body after its inlined style");
    let behavior_pos = body
        .find("Current behavior.")
        .expect("behavior prose renders");
    if let Some(details_pos) = body.find("<details") {
        assert!(
            behavior_pos < details_pos,
            "behavior must render before any <details> fold"
        );
    }
    // The earlier two-column contrast / two-cell table are gone; the figure still
    // leads, now as a readable structured Markdown fallback (states figure).
    assert!(!with_figure.html.contains("behavior-ba") && !with_figure.html.contains("ba-before"));
    assert!(!with_figure.markdown.contains("| Before | After |"));
    assert!(
        with_figure.markdown.contains("**States**"),
        "md behavior leads with the structured figure fallback: {}",
        with_figure.markdown
    );
    assert!(with_figure.markdown.contains("- Current behavior:"));
}

#[test]
fn the_behavior_keeps_full_width_prose_when_no_figure_is_authored() {
    let without = render_document(&final_plan_document());
    assert!(
        !without.html.contains("dg-figure behavior-figure"),
        "no behavior figure is invented when the plan authored none"
    );
    assert!(
        !without.html.contains("behavior-detail"),
        "behavior is never folded, with or without a figure"
    );
    assert!(
        without.html.contains("class=\"prose behavior-prose\""),
        "the behavior prose stays full-width"
    );
    assert!(without.html.contains("Current behavior."));
}

#[test]
fn both_a_current_and_a_target_flow_figure_are_hoisted() {
    // Two authored flow figures (a current-flow and a target-flow) both dominate
    // the behavior section and are de-duped from their narrative section.
    let mut candidate = dense_candidate();
    candidate["content"]["sections"] = json!([{
        "heading": "Flow",
        "blocks": [
            { "kind": "figure", "diagram": { "kind": "flow", "flow": {
                "orientation": "horizontal",
                "steps": [{ "name": "Inline call", "note": null }] } } },
            { "kind": "figure", "diagram": { "kind": "flow", "flow": {
                "orientation": "horizontal",
                "steps": [{ "name": "Enqueue then drain", "note": null }] } } }
        ]
    }]);
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated", "findings": [], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": candidate, "history": [round], "accepted": [], "comments": [] }),
    );
    let rendered = render_document(&doc);
    assert_eq!(
        rendered.html.matches("dg-figure behavior-figure").count(),
        2,
        "both flow figures are hoisted into the behavior section"
    );
    assert!(rendered.html.contains("Inline call") && rendered.html.contains("Enqueue then drain"));
    // De-duped: the emptied "Flow" section does not render a second copy.
    assert!(
        !rendered.html.contains(">Flow<"),
        "the emptied narrative section is dropped"
    );
}

#[test]
fn a_behavior_figure_carrying_an_open_comment_is_left_in_place_not_hoisted() {
    // F2: hoisting a behavior figure must never strip an open comment anchored to
    // its block. A flow figure with an unresolved comment stays in its own
    // section (not hoisted to the behavior header), so its inline comment anchor
    // survives in the render.
    let mut candidate = dense_candidate();
    candidate["content"]["sections"] = json!([{
        "heading": "Flow",
        "blocks": [
            { "kind": "figure", "diagram": { "kind": "flow", "flow": {
                "orientation": "horizontal",
                "steps": [{ "name": "Inline call", "note": null }] } } }
        ]
    }]);
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated", "findings": [], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": candidate, "history": [round], "accepted": [],
                "comments": [{ "anchor": { "section": 0, "block": 0 },
                               "text": "does this still lock?", "resolved": false }] }),
    );
    let rendered = render_document(&doc);
    // The commented figure is not hoisted to head the behavior area...
    assert_eq!(
        rendered.html.matches("dg-figure behavior-figure").count(),
        0,
        "a figure carrying an open comment is left in place, not hoisted: {}",
        rendered.html
    );
    // ...and its inline comment anchor is preserved rather than dropped.
    assert!(
        rendered.html.contains("revy-comment") && rendered.html.contains("does this still lock?"),
        "the open comment's inline anchor survives: {}",
        rendered.html
    );
}

#[test]
fn a_control_flow_figure_in_a_later_section_is_not_mistaken_for_the_behavior() {
    // F3: only the first section that authors a control-flow figure is the
    // behavior. A flow/state figure in a later section — an unrelated protocol
    // lifecycle — keeps its own section and is never hoisted or emptied.
    let mut candidate = dense_candidate();
    candidate["content"]["sections"] = json!([
        {
            "heading": "Flow",
            "blocks": [
                { "kind": "figure", "diagram": { "kind": "flow", "flow": {
                    "orientation": "horizontal",
                    "steps": [{ "name": "Behavior step", "note": null }] } } }
            ]
        },
        {
            "heading": "Protocol lifecycle",
            "blocks": [
                { "kind": "figure", "diagram": { "kind": "states", "states": [
                    { "name": "Handshake", "kind": null, "note": null }], "transitions": [] } }
            ]
        }
    ]);
    let round = json!({
        "wave": 1,
        "reviews": [{ "kind": "integrated", "findings": [], "lanes": [], "consumers": [] }],
        "triage": [], "rulings": []
    });
    let doc = document(
        "final",
        json!({ "candidate": candidate, "history": [round], "accepted": [], "comments": [] }),
    );
    let rendered = render_document(&doc);
    // Only the behavior section's figure is hoisted.
    assert_eq!(
        rendered.html.matches("dg-figure behavior-figure").count(),
        1,
        "only the first behavior-figure section is hoisted: {}",
        rendered.html
    );
    // The later section keeps its heading and its own figure — not emptied.
    assert!(
        rendered.html.contains("Protocol lifecycle") && rendered.html.contains("Handshake"),
        "an unrelated later control-flow figure keeps its own section: {}",
        rendered.html
    );
}

#[test]
fn a_decision_leads_with_the_question_and_reads_rejections_as_plain_text() {
    let rendered = render_document(&dense_final());
    let html = &rendered.html;
    // Item 5/6: the question is the bold headline; the slug id is a caption.
    assert!(
        html.contains("Decision: "),
        "decision kind tag + question lead"
    );
    // Item 4/5: each rejected alternative and its reason sit on their own lines,
    // with no strikethrough (the only rule that used it was `.ro-alt`).
    assert!(html.contains("ro-why"), "reason on its own line");
    assert!(
        !html.contains("text-decoration-color"),
        "no strikethrough on ruled-out alternatives"
    );
    // Markdown mirror: question bold on its own line, answer on the next.
    assert!(
        rendered.markdown.contains("**Decision: Which bound?**"),
        "markdown decision headline: {}",
        rendered.markdown
    );
}

#[test]
fn the_review_timeline_folds_into_a_one_line_digest() {
    let rendered = render_document(&dense_final());
    // Item 1: the retrospective timeline collapses into <details>; the digest
    // beside the heading carries the tally.
    assert!(
        rendered.html.contains("timeline-fold"),
        "folded into <details>"
    );
    assert!(
        rendered.html.contains("tl-digest"),
        "digest beside the heading"
    );
    assert!(
        rendered.html.contains("1 advisory"),
        "digest tallies the advisory finding: {}",
        rendered.html
    );
    // Markdown mirror uses the same <details>/summary and keeps the findings.
    assert!(
        rendered
            .markdown
            .contains("<details><summary>Review timeline —"),
        "markdown folded timeline: {}",
        rendered.markdown
    );
    assert!(rendered.markdown.contains("</details>"));
    assert!(
        rendered.markdown.contains("Consider a metric."),
        "the finding survives the fold"
    );
}

#[test]
fn the_markdown_mirrors_the_derived_figures() {
    let rendered = render_document(&dense_final());
    let markdown = &rendered.markdown;
    for section in [
        "Evidence map",
        "Changed symbols",
        "As-is call graph",
        "Review timeline",
    ] {
        assert!(
            markdown.contains(section),
            "markdown must carry `{section}`"
        );
    }
}
