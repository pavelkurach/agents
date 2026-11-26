//! The summary/appendix split (design §9): the authored plan renders first, the
//! derived figures follow in one appendix, and a section that would render the
//! same entities twice is folded into one.
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

/// A candidate with a limited claim, an evidence limitation cited by two claims,
/// an uncertainty, and two merge requests in dependency order.
fn folded_candidate() -> Value {
    let mut candidate = simple_candidate();
    candidate["exploration"]["evidence"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "kind": "semantic", "id": "evidence-calls", "provenance": { "tool": "serena", "version": null },
            "operation": "find_referencing_symbols", "subject": "push",
            "observation": { "shape": "call_edges", "edges": [
                { "caller": { "path": "src/api.rs", "line": 10, "column": 1 },
                  "callee": { "path": "src/queue.rs", "line": 5, "column": 1 } }
            ], "completeness": { "kind": "complete" } },
            "target": { "revision": "rev1", "dirty_digest": DIGEST },
            "limitation": "Only the api crate was indexed."
        }));
    candidate["exploration"]["claims"] = json!([
        {
            "id": "claim-queue", "statement": "The queue is bounded.",
            "evidence": ["evidence-queue", "evidence-calls"],
            "consequences": ["Producers block when full."],
            "conclusion": { "state": "limited", "limitation": "Only the push path was read." }
        },
        {
            "id": "claim-callers", "statement": "Only the api calls push.",
            "evidence": ["evidence-calls"], "consequences": ["One caller changes."],
            "conclusion": { "state": "established" }
        }
    ]);
    candidate["exploration"]["uncertainties"] = json!([{
        "id": "uncertainty-bound", "question": "Is the bound enough under burst load?",
        "disposition": { "state": "contained_by_criterion", "criterion": "ac-1" }
    }]);
    candidate["delivery"]["acceptance_criteria"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "ac-2", "statement": "The worker drains in FIFO order.",
            "verify": { "kind": "manual", "procedure": "Push 3, pop 3." }
        }));
    candidate["delivery"]["mrs"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id": "mr-2", "title": "Drain the queue from the worker",
            "files": ["src/queue.rs"], "needs": ["mr-1"], "satisfies": ["ac-2"],
            "standalone_behavior": "The worker pool drains the queue.", "verify": null
        }));
    candidate
}

fn folded_final(comments: Value) -> Value {
    document(
        "final",
        json!({ "candidate": folded_candidate(), "history": [],
                "accepted": [], "comments": comments }),
    )
}

#[test]
fn unresolved_facts_render_with_the_authored_plan_before_the_appendix() {
    let rendered = render_document(&folded_final(json!([])));

    let (authored, _) = rendered
        .markdown
        .split_once("\n## Appendix\n")
        .expect("one Appendix heading");

    assert!(
        authored.contains("## Unresolved facts and remedies"),
        "{}",
        rendered.markdown
    );
}

#[test]
fn a_limited_claim_carries_its_limitation_on_its_evidence_map_row() {
    let rendered = render_document(&folded_final(json!([])));

    assert!(
        rendered.markdown.contains(
            "- **Claim: The queue is bounded.**\n  claim-queue · evidence-queue (Documentation), \
             evidence-calls (Semantic, limited)\n  Limited: Only the push path was read.\n"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn an_evidence_limitation_cited_by_two_claims_is_listed_once() {
    let rendered = render_document(&folded_final(json!([])));

    assert_eq!(
        rendered
            .markdown
            .matches("Only the api crate was indexed.")
            .count(),
        1,
        "{}",
        rendered.markdown
    );
    assert!(
        rendered.markdown.contains(
            "Evidence limitations:\n- evidence-calls (Semantic): Only the api crate was indexed.\n"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn no_separate_evidence_limitations_section_repeats_the_claims() {
    let rendered = render_document(&folded_final(json!([])));

    assert!(
        !rendered.markdown.contains("Claim claim-queue:"),
        "{}",
        rendered.markdown
    );
    assert!(
        !rendered.markdown.contains("## Evidence limitations"),
        "{}",
        rendered.markdown
    );
}

#[test]
fn a_covered_lane_renders_on_the_row_of_the_subject_it_covers() {
    let rendered = render_document(&folded_final(json!([])));

    assert!(
        rendered.markdown.contains(
            "- **queue** [affected · behavioral]: Callers change. (refs: claim-queue · lanes: tests)\n"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn lanes_set_aside_for_one_reason_collapse_into_one_line() {
    let rendered = render_document(&folded_final(json!([])));

    assert!(
        rendered.markdown.contains(
            "- Not considered — API, data at rest, concurrency, resource bounds, observability, \
             compatibility, configuration, security: No change.\n"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn merge_requests_group_into_dependency_stages() {
    let rendered = render_document(&folded_final(json!([])));

    assert!(
        rendered
            .markdown
            .contains("- Stage 2\n  - **MR: Drain the queue from the worker**\n"),
        "{}",
        rendered.markdown
    );
    assert!(
        rendered.html.contains("<p class=\"mr-stage\">Stage 2</p>"),
        "{}",
        rendered.html
    );
}

#[test]
fn a_single_stage_delivery_lists_merge_requests_without_a_stage_label() {
    let rendered = render_document(&final_plan_document());

    assert!(
        !rendered.markdown.contains("Stage 1"),
        "{}",
        rendered.markdown
    );
}

#[test]
fn a_resolved_comment_renders_under_the_appendix_discussion() {
    let comments = json!([{ "anchor": { "section": 0, "block": 0 },
                            "text": "Looks right now.", "resolved": true }]);

    let rendered = render_document(&folded_final(comments));

    let (_, appendix) = rendered
        .markdown
        .split_once("\n## Appendix\n")
        .expect("one Appendix heading");
    assert!(
        appendix.contains("### Discussion\n\n- Overview — Looks right now.\n"),
        "{}",
        rendered.markdown
    );
}

/// A final plan whose candidate records one failed approved tool, redrafted once
/// and revised once.
fn reworked_final_document() -> Value {
    let mut candidate = simple_candidate();
    candidate["exploration"]["tool_failures"] =
        json!([{ "proposal": "proposal-serena", "reason": "the server crashed on startup" }]);
    let round = |wave: u32| {
        json!({ "wave": wave, "reviews": [common::clean_integrated_review("tests")],
                "triage": [], "rulings": [] })
    };
    document(
        "final",
        json!({
            "candidate": candidate, "history": [round(1), round(2)], "accepted": [], "comments": [],
            "lineage": {
                "reopens": [{ "target": "draft", "reason": "Split the MR.", "history": [round(1)] }],
                "revisions": [{ "wave": 2, "note": "Tighten the summary.", "addresses": ["finding-1"] }]
            }
        }),
    )
}

#[test]
fn a_failed_approved_tool_renders_in_the_appendix() {
    let rendered = render_document(&reworked_final_document());

    assert!(
        rendered.markdown.contains(
            "Tool failures:\n- approved tool proposal-serena failed: the server crashed on startup"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn the_timeline_marks_a_reopen_and_a_revision_in_order() {
    let rendered = render_document(&reworked_final_document());

    let timeline = rendered
        .markdown
        .split("### Review timeline")
        .nth(1)
        .expect("a timeline");
    let lines: Vec<&str> = timeline
        .lines()
        .filter(|line| line.starts_with("- "))
        .collect();
    assert_eq!(
        lines,
        [
            "- Wave 1 (Integrated)",
            "- **Reopened draft** — Split the MR.",
            "- Wave 1 (Integrated)",
            "- **Revised → wave 2** — Tighten the summary. (addresses finding-1)",
            "- Wave 2 (Integrated)",
        ]
    );
}

fn commented_rework_document() -> Value {
    let mut value = reworked_final_document();
    value["run"]["data"]["lineage"]["reopens"][0]["comments"] =
        json!([{ "anchor": { "section": 0, "block": 0 }, "text": "Name the bound." }]);
    value["run"]["data"]["lineage"]["revisions"][0]["decision_changes"] = json!([
        { "decision": "decision-bound", "field": "answer", "before": "1024", "after": "4096" }
    ]);
    value
}

#[test]
fn the_timeline_lists_the_comments_a_reopen_set_aside() {
    let rendered = render_document(&commented_rework_document());

    assert!(
        rendered.markdown.contains(
            "- **Reopened draft** — Split the MR.\n  - comment on s0b0: Name the bound.\n"
        ),
        "{}",
        rendered.markdown
    );
}

#[test]
fn the_timeline_shows_a_revision_changing_a_user_decision() {
    let rendered = render_document(&commented_rework_document());

    assert!(
        rendered
            .markdown
            .contains("  - decision `decision-bound` answer: 1024 → 4096\n"),
        "{}",
        rendered.markdown
    );
}

#[test]
fn the_html_timeline_lists_the_reopen_comments() {
    let rendered = render_document(&commented_rework_document());

    assert!(rendered.html.contains("Name the bound."));
}

#[test]
fn the_html_timeline_carries_the_revision_marker() {
    let rendered = render_document(&reworked_final_document());

    assert!(rendered.html.contains("Revised → wave 2"));
}
