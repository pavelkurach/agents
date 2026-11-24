//! Authored `Block::Figure` diagrams and code excerpts still render as real rich
//! markup — the redesign keeps them for flows, sequences, and states (design §9,
//! §10 decision 10).
#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::needless_pass_by_value,
    clippy::unwrap_used,
    clippy::expect_used
)]

mod common;

use common::{render_document, reviewing_document, simple_candidate};
use serde_json::{Value, json};

fn candidate_with_narrative() -> Value {
    let mut candidate = simple_candidate();
    candidate["content"]["orientation"] = json!({
        "placement": "Between the API handler and the worker pool.",
        "touched_flows": ["submit-job"],
        "glossary": [{ "term": "worker", "definition": "A process draining the queue." }]
    });
    candidate["content"]["sections"] = json!([{
        "heading": "Queue flow",
        "blocks": [
            { "kind": "prose", "text": "The queue exposes a **bounded** push/pop API." },
            { "kind": "code", "excerpt": { "lang": "rust", "caption": "push",
              "code": "fn push(&self, item: Job) -> Result<(), Full> { todo!() }" } },
            { "kind": "figure", "diagram": { "kind": "flow", "flow": {
                "orientation": "horizontal",
                "steps": [{ "name": "Receive request", "note": null },
                          { "name": "Enqueue", "note": null }] } } }
        ]
    }]);
    candidate
}

fn render() -> revy_plan::render::RenderedPlan {
    render_document(&reviewing_document(
        candidate_with_narrative(),
        json!([{ "kind": "integrated" }]),
    ))
}

#[test]
fn an_authored_flow_figure_renders_as_real_markup_not_a_placeholder() {
    let rendered = render();
    assert!(
        rendered.html.contains("flow"),
        "the flow diagram renders its own markup"
    );
    assert!(
        !rendered.html.contains("dg-placeholder"),
        "no placeholder sentinel"
    );
    assert!(
        rendered.html.contains("Enqueue"),
        "the flow's step labels survive"
    );
}

#[test]
fn a_code_excerpt_renders_as_a_highlighted_block() {
    let rendered = render();
    assert!(
        rendered.html.contains("figure class=\"code\""),
        "the code figure is present"
    );
    assert!(rendered.html.contains("push"), "the code content survives");
    // Markdown keeps a fenced block.
    assert!(
        rendered.markdown.contains("```rust"),
        "markdown fences the excerpt"
    );
}

#[test]
fn the_orientation_and_glossary_render() {
    let rendered = render();
    assert!(rendered.html.contains("Orientation"));
    assert!(rendered.html.contains("worker"), "glossary term");
    assert!(rendered.markdown.contains("submit-job"), "touched flow");
}

#[test]
fn a_states_transition_note_is_centered_and_has_a_structured_md_fallback() {
    let mut candidate = simple_candidate();
    candidate["content"]["sections"] = json!([{
        "heading": "Lifecycle",
        "blocks": [{ "kind": "figure", "diagram": {
            "kind": "states",
            "states": [{ "name": "Rendering", "kind": null, "note": null },
                       { "name": "Watching", "kind": null, "note": null }],
            "transitions": [{ "from": "Rendering", "to": "Watching",
                "on": "re-render error", "kind": "error", "note": "log; keep watching" }]
        } }]
    }]);
    let rendered = render_document(&reviewing_document(
        candidate,
        json!([{ "kind": "integrated" }]),
    ));
    // The transition note renders in the states figure...
    assert!(
        rendered
            .html
            .contains("<span class=\"note\">log; keep watching</span>"),
        "the transition note renders: {}",
        rendered.html
    );
    // ...and the stylesheet centers a transition note rather than letting it hang left.
    assert!(
        rendered
            .html
            .contains(".tr-row .note { flex-basis: 100%; text-align: center; }"),
        "the transition-note centering rule is present"
    );
    // The Markdown mirror renders the states figure as a structured list, not a
    // bare [diagram: states] placeholder.
    assert!(
        rendered.markdown.contains("**States**")
            && rendered
                .markdown
                .contains("Rendering → Watching on re-render error — log; keep watching"),
        "structured states fallback in markdown: {}",
        rendered.markdown
    );
    assert!(!rendered.markdown.contains("[diagram: states]"));
}
