//! HTML rendering for the 16 authored [`Diagram`] kinds, and the
//! [`diagram_markup`] dispatcher every `Block::Figure` renders through.
//!
//! Every kind renders plain HTML/CSS ported verbatim from render.py's own
//! drawers — no fixed-geometry SVG, so the browser lays out and wraps every
//! label and nothing can overlap or clip: `Seq` (`seq.rs`), `Dag`/`Layers`/
//! `Flow`/`States`/`Timeline`/`Tree` (`graph.rs`), `Er`/`Endpoint`/`Matrix`/
//! `Compare`/`Stack` (`tables.rs`), and the proportional-bar family
//! `Layout`/`Budget`/`Trace` (`bars.rs`, `trace.rs`). See
//! `render.py:202-231,278-320,323-372,375-410,413-505,508-590,593-620,744-809,
//! 771-790,812-830` and the matching `.fl-*`/`.vf-*`/`.sq-*`/`.ly-*`/`.lye-*`/
//! `.stt-*`/`.tr-*`/`.wv-*`/`.tl-*`/`.tre-*`/`.er-*`/`.ep-*`/`.mx*`/`.compare`/
//! `.cmp-*`/`.stack*`/`.mark*`/`.lay-*`/`.bg-*`/`.ex*` in `plan.css`.
//! `diagram_markup`'s match is exhaustive over all 16 variants (`Examples`
//! renders through `tables.rs`). The only inline `<svg>` left anywhere is the
//! decorative [`Icon`] glyph set below.

mod bars;
mod graph;
mod seq;
mod tables;
mod trace;

use std::fmt::Write as _;

use maud::{Markup, html as markup};

use crate::model::diagram::{
    ActorIndex, Diagram, FlowOrientation, OutcomeKind, SeqBody, State, StepKind, Transition,
};
use crate::primitives::{NonEmptyText, OneOrMore};
use bars::{budget_markup, layout_markup};
use graph::{dag_markup, flow_markup, layers_markup, states_markup, timeline_markup, tree_markup};
use seq::seq_markup;
use tables::{
    compare_markup, endpoint_markup, er_markup, examples_markup, matrix_markup, stack_markup,
};
use trace::trace_markup;

/// A fixed, static icon glyph set — ported from `render.py`'s own `ICONS`
/// technique (copied path/shape data, no font, no network; presentation.md:29-32).
/// `Key`/`Link`/`Lock` are ported verbatim from `render.py`'s own `ICONS["key"]`/
/// `["link"]`/`["lock"]` path data (`render.py:239-252`), at the same `0 0 16
/// 16` canvas and stroke style `Flag` already uses — `tables.rs`'s `er` pk/fk
/// badges use `Key`/`Link`; `endpoint`'s auth chip reuses `Exit` for
/// `Auth::Open` (old Python's own choice, `render.py:560`) and `Lock` for
/// `Auth::Scheme`. `Plus`/`Cloud` (`FP2-HTML-2`) are ported the same way from
/// `ICONS["plus"]`/`["cloud"]` (`render.py:244-246`) — `graph.rs`'s
/// `chip_markup` uses them for a `layers` node chip's `new`/`external` kind
/// glyph, matching render.py's own `chip()`/`CHIP_ICONS` (`render.py:270-275`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    Alert,
    Cloud,
    Exit,
    Flag,
    Key,
    Link,
    Lock,
    Plus,
}

impl Icon {
    fn markup(self) -> Markup {
        match self {
            Self::Alert => markup! {
                svg class="dg-icon" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true" focusable="false" {
                    circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2" {}
                    line x1="12" y1="7" x2="12" y2="13" stroke="currentColor" stroke-width="2" stroke-linecap="round" {}
                    circle cx="12" cy="16.5" r="1" fill="currentColor" {}
                }
            },
            Self::Exit => markup! {
                svg class="dg-icon" viewBox="0 0 24 24" width="14" height="14" aria-hidden="true" focusable="false" {
                    path d="M9 4 H5 a1 1 0 0 0 -1 1 v14 a1 1 0 0 0 1 1 h4" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" {}
                    line x1="21" y1="12" x2="9" y2="12" stroke="currentColor" stroke-width="2" stroke-linecap="round" {}
                    path d="M16 7 L21 12 L16 17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" {}
                }
            },
            Self::Plus => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
                    path d="M8 3.2v9.6M3.2 8h9.6" {}
                }
            },
            Self::Cloud => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
                    path d="M4.6 12.4h6.9a2.6 2.6 0 0 0 .2-5.2 3.6 3.6 0 0 0-6.9-1 2.6 2.6 0 0 0-.2 6.2z" {}
                }
            },
            Self::Flag => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" focusable="false" {
                    path d="M4 14V3" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" {}
                    path d="M4 3.6h8l-1.8 2.6L12 8.8H4z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" {}
                }
            },
            Self::Key => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
                    circle cx="5" cy="11" r="2.6" {}
                    path d="M6.9 9.1 14 2m-2.2 1.8 1.8 1.8m-4 .2 1.8 1.8" {}
                }
            },
            Self::Link => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
                    path d="M6.6 9.4a3 3 0 0 0 4.3 0l2-2a3 3 0 0 0-4.3-4.3l-1 1" {}
                    path d="M9.4 6.6a3 3 0 0 0-4.3 0l-2 2a3 3 0 0 0 4.3 4.3l1-1" {}
                }
            },
            Self::Lock => markup! {
                svg class="dg-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false" {
                    rect x="3.2" y="7" width="9.6" height="6.6" rx="1.2" {}
                    path d="M5.6 7V5.2a2.4 2.4 0 0 1 4.8 0V7" {}
                }
            },
        }
    }
}

/// The dispatcher every `Block::Figure` renders through, exhaustive over all
/// 16 [`Diagram`] variants — every one has a real HTML drawer (design §5).
pub(super) fn diagram_markup(diagram: &Diagram) -> Markup {
    match diagram {
        Diagram::Layers { layers, edges } => layers_markup(layers, edges),
        Diagram::Flow { flow } => flow_markup(flow),
        Diagram::Seq { body } => seq_markup(body),
        Diagram::Er {
            entities,
            relations,
        } => er_markup(entities, relations),
        Diagram::Endpoint {
            method,
            path,
            summary,
            auth,
            params,
            request,
            responses,
        } => endpoint_markup(
            *method,
            path,
            summary.as_ref(),
            auth,
            params,
            request.as_ref(),
            responses,
        ),
        Diagram::States {
            states,
            transitions,
        } => states_markup(states, transitions),
        Diagram::Dag {
            caption,
            waves,
            edges,
        } => dag_markup(caption.as_ref(), waves, edges),
        Diagram::Layout {
            caption,
            unit,
            fields,
        } => layout_markup(caption.as_ref(), unit, fields),
        Diagram::Timeline { steps } => timeline_markup(steps),
        Diagram::Tree { entries } => tree_markup(entries),
        Diagram::Matrix { body } => matrix_markup(body),
        Diagram::Trace {
            caption,
            legend,
            ops,
        } => trace_markup(caption.as_ref(), legend, ops),
        Diagram::Budget {
            budget,
            unit,
            parts,
        } => budget_markup(*budget, unit, parts),
        Diagram::Compare { columns } => compare_markup(columns),
        Diagram::Stack { frames } => stack_markup(frames),
        Diagram::Examples { caption, rows } => examples_markup(caption.as_ref(), rows),
    }
}

/// The Markdown fallback for a figure. Flow, sequence, and state figures — the
/// control-flow shapes a plan leads with — render as a readable nested list
/// derived from the same typed data the HTML draws, so a `plan.md` reader sees
/// the flow without SVG. Every other kind keeps a compact `[diagram: <kind>]`
/// line.
pub(super) fn diagram_markdown_fallback(diagram: &Diagram) -> String {
    match diagram {
        Diagram::Flow { flow } => flow_fallback(flow),
        Diagram::Seq { body } => seq_fallback(body),
        Diagram::States {
            states,
            transitions,
        } => states_fallback(states, transitions),
        _ => format!("[diagram: {}]", diagram.kind_name()),
    }
}

fn flow_fallback(flow: &FlowOrientation) -> String {
    let mut out = String::from("**Flow**\n\n");
    match flow {
        FlowOrientation::Horizontal { steps } => {
            for (index, step) in steps.as_slice().iter().enumerate() {
                let _ = writeln!(
                    out,
                    "{}. {}{}",
                    index + 1,
                    step.name.as_str(),
                    note_suffix(step.note.as_ref())
                );
            }
        }
        FlowOrientation::Vertical { steps } => {
            for (index, step) in steps.as_slice().iter().enumerate() {
                let _ = writeln!(
                    out,
                    "{}. {}{}",
                    index + 1,
                    step.name.as_str(),
                    note_suffix(step.note.as_ref())
                );
                for outcome in &step.outcomes {
                    let _ = writeln!(
                        out,
                        "   - if {} → {} ({})",
                        outcome.when.as_str(),
                        outcome.then.as_str(),
                        outcome_kind_word(outcome.kind)
                    );
                }
            }
        }
    }
    out.trim_end().to_owned()
}

fn seq_fallback(body: &SeqBody) -> String {
    let actors = body.actors.as_slice();
    let name = |index: ActorIndex| {
        actors
            .get(index.as_usize())
            .map_or("?", |actor| actor.name.as_str())
    };
    let mut out = String::from("**Sequence**\n\n");
    for (index, step) in body.steps.iter().enumerate() {
        let kind = step
            .kind
            .map(step_kind_word)
            .map_or_else(String::new, |word| format!(" ({word})"));
        let _ = writeln!(
            out,
            "{}. {} → {}: {}{}{}",
            index + 1,
            name(step.from),
            name(step.to),
            step.label.as_str(),
            kind,
            note_suffix(step.note.as_ref())
        );
    }
    out.trim_end().to_owned()
}

fn states_fallback(states: &OneOrMore<State>, transitions: &[Transition]) -> String {
    let names = states
        .as_slice()
        .iter()
        .map(|state| state.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = format!("**States**\n\n- States: {names}\n");
    if !transitions.is_empty() {
        out.push_str("- Transitions:\n");
        for transition in transitions {
            let on = transition
                .on
                .as_ref()
                .map_or_else(String::new, |on| format!(" on {}", on.as_str()));
            let _ = writeln!(
                out,
                "  - {} → {}{}{}",
                transition.from.as_str(),
                transition.to.as_str(),
                on,
                note_suffix(transition.note.as_ref())
            );
        }
    }
    out.trim_end().to_owned()
}

fn note_suffix(note: Option<&NonEmptyText>) -> String {
    note.map_or_else(String::new, |note| format!(" — {}", note.as_str()))
}

const fn outcome_kind_word(kind: OutcomeKind) -> &'static str {
    match kind {
        OutcomeKind::Error => "error",
        OutcomeKind::Exit => "exit",
    }
}

const fn step_kind_word(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Return => "return",
        StepKind::SelfCall => "self",
        StepKind::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::too_many_lines
    )]
    use super::*;

    /// Minimal valid JSON for every one of the 16 [`Diagram`] kinds — used to
    /// prove the complete dispatcher: no kind falls back to a placeholder any
    /// more (design §5's "delete `placeholder`" step).
    fn minimal_json_for_every_kind() -> Vec<(&'static str, serde_json::Value)> {
        vec![
            (
                "layers",
                serde_json::json!({
                    "kind": "layers",
                    "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": "new"}]}],
                    "edges": []
                }),
            ),
            (
                "flow",
                serde_json::json!({
                    "kind": "flow",
                    "flow": {
                        "orientation": "horizontal",
                        "steps": [{"name": "Receive request", "note": null}]
                    }
                }),
            ),
            (
                "seq",
                serde_json::json!({
                    "kind": "seq",
                    "actors": [{"name": "A", "note": null}],
                    "steps": [],
                    "spans": []
                }),
            ),
            (
                "er",
                serde_json::json!({
                    "kind": "er",
                    "entities": [{
                        "name": "Order",
                        "kind": null,
                        "fields": [{"name": "id", "type": "uuid", "key": {"kind": "primary"}}]
                    }],
                    "relations": []
                }),
            ),
            (
                "endpoint",
                serde_json::json!({
                    "kind": "endpoint",
                    "method": "get",
                    "path": "/orders",
                    "summary": null,
                    "auth": {"kind": "open", "reason": "public read"},
                    "params": [],
                    "request": null,
                    "responses": [{
                        "status": 200,
                        "description": "ok",
                        "kind": null,
                        "headers": [],
                        "body": {"content_type": null, "example": null}
                    }]
                }),
            ),
            (
                "dag",
                serde_json::json!({
                    "kind": "dag",
                    "caption": null,
                    "waves": [{
                        "name": null,
                        "mrs": [{"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                        "gate": null
                    }],
                    "edges": []
                }),
            ),
            (
                "states",
                serde_json::json!({
                    "kind": "states",
                    "states": [{"name": "Idle", "kind": null, "note": null}],
                    "transitions": []
                }),
            ),
            (
                "layout",
                serde_json::json!({
                    "kind": "layout",
                    "caption": null,
                    "unit": "B",
                    "fields": [{"name": "header", "size": 4, "kind": null, "note": null}]
                }),
            ),
            (
                "timeline",
                serde_json::json!({
                    "kind": "timeline",
                    "steps": [{"name": "Deploy", "kind": null, "wait": null, "rollback": null, "note": null}]
                }),
            ),
            (
                "tree",
                serde_json::json!({
                    "kind": "tree",
                    "entries": [{"path": "src/lib.rs", "change": null, "note": null}]
                }),
            ),
            (
                "matrix",
                serde_json::json!({
                    "kind": "matrix",
                    "columns": ["dev", "prod"],
                    "rows": [{"name": "timeout", "cells": [{"column": "prod", "value": "30s", "kind": null}]}]
                }),
            ),
            (
                "trace",
                serde_json::json!({
                    "kind": "trace",
                    "caption": null,
                    "legend": [],
                    "ops": [{"op": "W1", "key": "page-4", "kind": null}]
                }),
            ),
            (
                "budget",
                serde_json::json!({
                    "kind": "budget",
                    "budget": 100.0,
                    "unit": "ms",
                    "parts": [{"name": "db", "value": 40.0, "kind": null}]
                }),
            ),
            (
                "compare",
                serde_json::json!({
                    "kind": "compare",
                    "columns": [{"label": "before", "items": [{"text": "inline", "kind": null}]}]
                }),
            ),
            (
                "stack",
                serde_json::json!({
                    "kind": "stack",
                    "frames": [{"marker": null, "symbol": "main", "note": null, "children": []}]
                }),
            ),
            (
                "examples",
                serde_json::json!({
                    "kind": "examples",
                    "caption": null,
                    "rows": [{"given": null, "when": "LPUSH k a b c", "then": "c,b,a", "kind": "wire"}]
                }),
            ),
        ]
    }

    #[test]
    fn test_every_kind_renders_real_content_with_no_placeholder_text() {
        for (kind, json) in minimal_json_for_every_kind() {
            let diagram: Diagram = serde_json::from_value(json).unwrap_or_else(|error| {
                panic!("`{kind}` must parse from its minimal JSON: {error}")
            });

            let markup = diagram_markup(&diagram).into_string();

            assert!(
                !markup.contains("not yet rendered") && !markup.contains("dg-placeholder"),
                "`{kind}` must render real content, not a placeholder: {markup}"
            );
            assert!(!markup.is_empty(), "`{kind}` must render non-empty markup");
        }
    }

    #[test]
    fn test_every_kind_names_itself_in_its_own_minimal_fixture() {
        // Guards the fixture list above against silently testing the wrong
        // kind (e.g. a copy-paste that left `"kind"` stale).
        for (kind, json) in minimal_json_for_every_kind() {
            let diagram: Diagram = serde_json::from_value(json).unwrap_or_else(|error| {
                panic!("`{kind}` must parse from its minimal JSON: {error}")
            });
            assert_eq!(diagram.kind_name(), kind);
        }
    }

    #[test]
    fn test_diagram_markdown_fallback_names_a_kind_without_a_structured_form() {
        let diagram: Diagram = serde_json::from_value(serde_json::json!({
            "kind": "timeline",
            "steps": [{"name": "Deploy", "kind": null, "wait": null, "rollback": null, "note": null}]
        }))
        .expect("valid timeline");

        assert_eq!(diagram_markdown_fallback(&diagram), "[diagram: timeline]");
    }

    #[test]
    fn test_flow_fallback_lists_its_steps_as_a_numbered_list() {
        let diagram: Diagram = serde_json::from_value(serde_json::json!({
            "kind": "flow",
            "flow": { "orientation": "horizontal", "steps": [
                {"name": "Receive request", "note": null},
                {"name": "Enqueue", "note": "bounded"}] }
        }))
        .expect("valid flow");
        let md = diagram_markdown_fallback(&diagram);
        assert!(md.contains("**Flow**"), "{md}");
        assert!(md.contains("1. Receive request"), "{md}");
        assert!(md.contains("2. Enqueue — bounded"), "{md}");
        assert!(
            !md.contains("[diagram:"),
            "structured, not a placeholder: {md}"
        );
    }

    #[test]
    fn test_states_fallback_lists_transitions_with_their_labels() {
        let diagram: Diagram = serde_json::from_value(serde_json::json!({
            "kind": "states",
            "states": [{"name": "Rendering", "kind": null, "note": null},
                       {"name": "Watching", "kind": null, "note": null}],
            "transitions": [{"from": "Rendering", "to": "Watching",
                "on": "re-render error", "kind": "error", "note": "log; keep watching"}]
        }))
        .expect("valid states");
        let md = diagram_markdown_fallback(&diagram);
        assert!(md.contains("**States**"), "{md}");
        assert!(md.contains("- States: Rendering, Watching"), "{md}");
        assert!(
            md.contains("Rendering → Watching on re-render error — log; keep watching"),
            "{md}"
        );
    }

    #[test]
    fn test_seq_fallback_resolves_actor_names_for_each_step() {
        let diagram: Diagram = serde_json::from_value(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "Client", "note": null}, {"name": "Server", "note": null}],
            "steps": [{"from": "Client", "to": "Server", "label": "GET /x", "kind": null, "note": null}],
            "spans": []
        }))
        .expect("valid seq");
        let md = diagram_markdown_fallback(&diagram);
        assert!(md.contains("**Sequence**"), "{md}");
        assert!(md.contains("1. Client → Server: GET /x"), "{md}");
    }
}
