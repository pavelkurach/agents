//! `revy:trace` — a measured run of operations, in lanes (design §1.12).
//! Ported to render.py's HTML (`d_trace`, FP2-HTML): the `.d.trace` container
//! is a flex column carrying `--n` (the op count) — one `.tr-lane` per
//! first-seen distinct `key`, each a `.tr-cells` grid that inherits `--n` for
//! its column count. A filled `.tr-c.on` cell marks where the op acts on that
//! lane, an empty `.tr-c` otherwise; the caption is a `.d-cap` sibling outside
//! the container, exactly as `render.py` emits it. The browser sizes the grid.

use maud::{Markup, html as markup};

use crate::model::diagram::{LegendEntry, TraceOp, TraceOpKind};
use crate::primitives::{NonEmptyText, OneOrMore};

pub(super) fn trace_markup(
    caption: Option<&NonEmptyText>,
    legend: &[LegendEntry],
    ops: &OneOrMore<TraceOp>,
) -> Markup {
    let ops = ops.as_slice();
    let lanes = first_seen_lanes(ops);

    markup! {
        div class="d trace" style=(format!("--n:{}", ops.len().max(1))) {
            @for lane in &lanes {
                div class="tr-lane" {
                    div class="tr-k" { (lane.as_str()) }
                    div class="tr-cells" {
                        @for op in ops {
                            (cell(op, lane))
                        }
                    }
                }
            }
            @if !legend.is_empty() {
                div class="tr-leg" {
                    @for entry in legend {
                        span class="tr-l" { b { (entry.symbol.as_str()) } " " (entry.meaning.as_str()) }
                    }
                }
            }
        }
        @if let Some(caption) = caption {
            div class="d-cap" { (caption.as_str()) }
        }
    }
}

/// Distinct `key`s, first-seen order — one lane per thing acted on.
fn first_seen_lanes(ops: &[TraceOp]) -> Vec<&NonEmptyText> {
    let mut lanes: Vec<&NonEmptyText> = Vec::new();
    for op in ops {
        if !lanes.iter().any(|lane| lane.as_str() == op.key.as_str()) {
            lanes.push(&op.key);
        }
    }
    lanes
}

fn cell(op: &TraceOp, lane: &NonEmptyText) -> Markup {
    markup! {
        @if op.key.as_str() == lane.as_str() {
            div class=(format!("tr-c on{}", cell_kind_suffix(op.kind))) title=(format!("{} {}", op.op.as_str(), lane.as_str())) {
                (op.op.as_str())
            }
        } @else {
            div class="tr-c" {}
        }
    }
}

const fn cell_kind_suffix(kind: Option<TraceOpKind>) -> &'static str {
    match kind {
        Some(TraceOpKind::Waste) => " waste",
        Some(TraceOpKind::Saved) => " saved",
        Some(TraceOpKind::New) => " new",
        None => "",
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
    use crate::render::diagram::diagram_markup;

    fn render(json: serde_json::Value) -> String {
        let diagram: Diagram = serde_json::from_value(json).expect("valid diagram JSON");
        diagram_markup(&diagram).into_string()
    }

    #[test]
    fn test_trace_two_ops_sharing_a_key_render_in_the_same_lane() {
        let html = render(serde_json::json!({
            "kind": "trace", "caption": null, "legend": [],
            "ops": [
                {"op": "W1", "key": "page-4", "kind": null},
                {"op": "R1", "key": "page-4", "kind": null}
            ]
        }));
        assert_eq!(html.matches("class=\"tr-lane\"").count(), 1);
        assert_eq!(html.matches("tr-c on").count(), 2);
        // render.py carries `--n` on the `.d.trace` container, not a wrapper —
        // `.tr-set` belongs to the states diagram and must not appear here.
        assert!(html.contains("class=\"d trace\" style=\"--n:2\""));
        assert!(!html.contains("tr-set"));
    }

    #[test]
    fn test_trace_a_distinct_key_opens_a_new_lane() {
        let html = render(serde_json::json!({
            "kind": "trace", "caption": null, "legend": [],
            "ops": [
                {"op": "W1", "key": "page-4", "kind": null},
                {"op": "R1", "key": "page-9", "kind": null}
            ]
        }));
        assert_eq!(html.matches("class=\"tr-lane\"").count(), 2);
    }

    #[test]
    fn test_trace_kind_colours_the_cell() {
        let html = render(serde_json::json!({
            "kind": "trace", "caption": null, "legend": [],
            "ops": [{"op": "W1", "key": "page-4", "kind": "waste"}]
        }));
        assert!(html.contains("tr-c on waste"));
    }

    #[test]
    fn test_trace_renders_the_legend_and_caption() {
        let html = render(serde_json::json!({
            "kind": "trace", "caption": "cold cache",
            "legend": [{"symbol": "W", "meaning": "write"}],
            "ops": [{"op": "W1", "key": "page-4", "kind": null}]
        }));
        assert!(html.contains("tr-leg"));
        assert!(html.contains("write"));
        assert!(html.contains("cold cache"));
    }

    #[test]
    fn test_rendering_the_same_trace_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "trace", "caption": null, "legend": [],
            "ops": [
                {"op": "W1", "key": "page-4", "kind": null},
                {"op": "R1", "key": "page-9", "kind": "new"}
            ]
        });
        assert_eq!(render(json.clone()), render(json));
    }
}
