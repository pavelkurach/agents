//! `revy:seq` — messages across a boundary (design §1.3, R2b).
//!
//! `FP2-HTML`: `render.py`'s `d_seq` (`render.py:323-372`) never computes
//! pixel geometry at all — it emits an HTML CSS-grid/absolute-% layout
//! (`.sq-*`, ported verbatim to `plan.css`) and lets the browser measure and
//! wrap text, which is why nothing in it ever overlapped or got cut off.
//! This drawer used to be SVG with its own text-width estimate to
//! approximate that effect (`FP2a`); it now ports render.py's markup
//! directly instead, so there is no pixel math left to get wrong. The
//! model's `TryFrom` actor-index resolution (`model/diagram.rs`'s
//! `RawSeqDiagram`) is unchanged — this file only turns already-resolved
//! [`ActorIndex`]es into the `%` positions render.py's own `d_seq` computes
//! at `render.py:339-361`.

use maud::{Markup, html as markup};

use crate::model::diagram::{Actor, SeqBody, Span, SpanKind, Step, StepKind};

pub(super) fn seq_markup(body: &SeqBody) -> Markup {
    let actors = body.actors.as_slice();
    // `actors` is a `OneOrMore`, so this is always >= 1 — matches
    // render.py's own `n = max(len(actors), 1)` floor (render.py:332).
    let n = actors.len();

    markup! {
        div class="d seq" style=(format!("--n:{n}")) {
            div class="sq-actors" {
                @for actor in actors {
                    (actor_header(actor))
                }
            }
            div class="sq-body" {
                @for index in 0..n {
                    div class="sq-life" style=(format!("left:{}%", fmt_pct(lifeline_left(index, n)))) {}
                }
                @for span in &body.spans {
                    (span_markup(span))
                }
                @for step in &body.steps {
                    (step_row(step, n))
                }
            }
        }
    }
}

fn actor_header(actor: &Actor) -> Markup {
    markup! {
        div class="sq-actor" {
            (actor.name.as_str())
            @if let Some(note) = &actor.note {
                span class="sq-anote" { (note.as_str()) }
            }
        }
    }
}

/// Widen a small actor index or count to `f64` for percent geometry. Actor
/// counts are tiny, so the `u32` clamp never triggers; the two-step conversion
/// keeps the widening lossless (`u32 -> f64`) rather than a precision-losing
/// `usize as f64`.
fn as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

/// A lifeline's `left:%` — render.py:339-341's `(i + 0.5) * 100 / n`.
fn lifeline_left(index: usize, n: usize) -> f64 {
    (as_f64(index) + 0.5) * 100.0 / as_f64(n)
}

/// A message span's `left:%` — render.py:350-352's `(lo + 0.5) * 100 / n`,
/// over the already-`min`ed endpoint.
fn message_left(lo: usize, n: usize) -> f64 {
    (as_f64(lo) + 0.5) * 100.0 / as_f64(n)
}

/// A message span's `width:%` — render.py:353's `max((hi - lo) * 100 / n,
/// 0.5)`; a self-call (`lo == hi`) floors to the same 0.5% sliver render.py
/// draws rather than vanishing.
fn message_width(lo: usize, hi: usize, n: usize) -> f64 {
    (as_f64(hi - lo) * 100.0 / as_f64(n)).max(0.5)
}

/// render.py formats every `%` position with `:.4f` (render.py:340,352,360)
/// — matched here so byte-identical rendering (this module's own
/// determinism test) holds across repeated runs.
fn fmt_pct(value: f64) -> String {
    format!("{value:.4}")
}

fn step_row(step: &Step, n: usize) -> Markup {
    let from = step.from.as_usize();
    let to = step.to.as_usize();
    let lo = from.min(to);
    let hi = from.max(to);
    let left = fmt_pct(message_left(lo, n));
    let width = fmt_pct(message_width(lo, hi, n));
    // render.py:354/357 — `back`/`self_call` are independent of `kind`:
    // `back` from which endpoint is earlier in `actors`, `self` from
    // `from == to`, so a `kind:"self"` step (`StepKind::SelfCall`, which
    // maps to the same `self` token below) and a same-actor step both add
    // it, matching render.py's own harmless class-token duplication.
    let back = to < from;
    let is_self = from == to;

    markup! {
        div class="sq-row" {
            div class=(message_class(back, is_self, step.kind)) style=(format!("left:{left}%;width:{width}%")) {
                span class="sq-label" { (step.label.as_str()) }
            }
            @if let Some(note) = &step.note {
                div class="sq-note" { (note.as_str()) }
            }
        }
    }
}

/// render.py:359's `f'sq-msg{back}{kind}{self_call}'` — same class-token
/// order, so a diff against a fixture reads as a diff, not a reorder.
fn message_class(back: bool, is_self: bool, kind: Option<StepKind>) -> String {
    let mut class = String::from("sq-msg");
    if back {
        class.push_str(" back");
    }
    if let Some(kind) = kind {
        class.push(' ');
        class.push_str(step_kind_token(kind));
    }
    if is_self {
        class.push_str(" self");
    }
    class
}

const fn step_kind_token(kind: StepKind) -> &'static str {
    match kind {
        StepKind::Return => "return",
        StepKind::SelfCall => "self",
        StepKind::Error => "error",
    }
}

fn span_markup(span: &Span) -> Markup {
    let class = format!("sq-span sq-{}", span_kind_token(span.kind));
    let style = format!("--a:{};--b:{}", span.from.as_usize(), span.to.as_usize());
    markup! {
        div class=(class) style=(style) {
            span class="sq-span-l" { (span.label.as_str()) }
        }
    }
}

const fn span_kind_token(kind: Option<SpanKind>) -> &'static str {
    match kind {
        Some(SpanKind::Lock) => "lock",
        // `SpanKind` defaults to `tx` when absent — cosmetic only, design §0.5.
        Some(SpanKind::Tx) | None => "tx",
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]
    use crate::model::diagram::Diagram;

    fn render(json: serde_json::Value) -> String {
        let diagram: Diagram = serde_json::from_value(json).expect("valid diagram JSON");
        super::super::diagram_markup(&diagram).into_string()
    }

    #[test]
    fn test_seq_renders_the_d_seq_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}],
            "steps": [],
            "spans": []
        }));

        assert!(html.contains("class=\"d seq\""));
        assert!(
            !html.contains("<svg"),
            "seq must no longer render an <svg> canvas: {html}"
        );
    }

    #[test]
    fn test_seq_error_step_renders_the_red_stroke_class() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "Client", "note": null}, {"name": "Server", "note": null}],
            "steps": [{"from": "Client", "to": "Server", "label": "GET /x", "kind": "error", "note": null}],
            "spans": []
        }));

        assert!(html.contains("class=\"sq-msg error\""));
    }

    #[test]
    fn test_seq_return_step_renders_the_return_and_back_classes() {
        // Server (index 1) -> Client (index 0): `to < from`, so `back` joins
        // `return` in class order, matching render.py's own back-then-kind
        // ordering (render.py:359).
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "Client", "note": null}, {"name": "Server", "note": null}],
            "steps": [{"from": "Server", "to": "Client", "label": "200 OK", "kind": "return", "note": null}],
            "spans": []
        }));

        assert!(html.contains("class=\"sq-msg back return\""));
    }

    #[test]
    fn test_seq_self_call_renders_the_self_class_with_a_minimal_width_sliver() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "Worker", "note": null}],
            "steps": [{"from": "Worker", "to": "Worker", "label": "retry", "kind": "self", "note": null}],
            "spans": []
        }));

        // `kind:"self"` and the auto `from == to` self-call both append the
        // `self` token — render.py's own harmless duplication (see
        // `message_class`'s doc comment).
        assert!(html.contains("class=\"sq-msg self self\""));
        assert!(html.contains("width:0.5000%"));
    }

    #[test]
    fn test_seq_span_renders_the_lock_class() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [],
            "spans": [{"from": 0, "to": 1, "label": "critical section", "kind": "lock"}]
        }));

        assert!(html.contains("class=\"sq-span sq-lock\""));
    }

    #[test]
    fn test_seq_span_defaults_to_the_tx_class_when_kind_is_absent() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [],
            "spans": [{"from": 0, "to": 1, "label": "tx", "kind": null}]
        }));

        assert!(html.contains("class=\"sq-span sq-tx\""));
    }

    #[test]
    fn test_seq_renders_one_lifeline_per_actor() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}, {"name": "C", "note": null}],
            "steps": [],
            "spans": []
        }));

        assert_eq!(html.matches("sq-life").count(), 3);
    }

    #[test]
    fn test_seq_lifeline_and_message_positions_are_percent_of_actor_count() {
        // Two actors: lifelines at 25%/75%; a 0->1 message spans the same
        // 25% start with a 50% width — render.py's own `(i+.5)*100/n` /
        // `(lo+.5)*100/n` / `max((hi-lo)*100/n, .5)` (render.py:340,352,353).
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [{"from": "A", "to": "B", "label": "req", "kind": null, "note": null}],
            "spans": []
        }));

        assert!(html.contains("left:25.0000%"));
        assert!(html.contains("left:75.0000%"));
        assert!(html.contains("left:25.0000%;width:50.0000%"));
    }

    #[test]
    fn test_seq_long_message_label_renders_intact_as_one_unwrapped_span() {
        // render.py never wraps `.sq-label` in Python or CSS
        // (`white-space:nowrap`, plan.css) — the browser, not this drawer,
        // decides how it overflows, so a long label is still exactly one
        // `sq-label` span with the full text inside it.
        let label = "this label is deliberately far too long to fit on one line without wrapping";
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [{"from": "A", "to": "B", "label": label, "kind": null, "note": null}],
            "spans": []
        }));

        assert_eq!(html.matches("sq-label").count(), 1);
        assert!(html.contains(label));
    }

    #[test]
    fn test_seq_step_note_renders_as_its_own_sq_note_line_not_appended_to_the_label() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [{"from": "A", "to": "B", "label": "push", "kind": null, "note": "bounded"}],
            "spans": []
        }));

        assert!(html.contains("sq-note"));
        assert!(html.contains(">push<"));
        assert!(html.contains(">bounded<"));
    }

    #[test]
    fn test_seq_actor_note_renders_inside_the_actor_header() {
        let html = render(serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "Queue", "note": "bounded, capacity 16"}],
            "steps": [],
            "spans": []
        }));

        assert!(html.contains(
            "class=\"sq-actor\">Queue<span class=\"sq-anote\">bounded, capacity 16</span>"
        ));
    }

    #[test]
    fn test_rendering_the_same_seq_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "seq",
            "actors": [{"name": "A", "note": null}, {"name": "B", "note": null}],
            "steps": [{"from": "A", "to": "B", "label": "req", "kind": null, "note": null}],
            "spans": [{"from": 0, "to": 1, "label": "tx", "kind": null}]
        });

        assert_eq!(render(json.clone()), render(json));
    }
}
