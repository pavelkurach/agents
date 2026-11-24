//! The proportional-bar family: `revy:layout` (design §1.8) and `revy:budget`
//! (design §1.13). Ported to render.py's HTML (`d_layout`/`d_budget`, FP2-HTML):
//! a flex `.lay-bar`/`.bg-bar` whose segments carry a `--g`/`--w` CSS custom
//! property (flex-grow), so the browser lays the bar out to scale — no SVG,
//! no pixel geometry, nothing to overflow.

use maud::{Markup, html as markup};

use crate::model::diagram::{BudgetPart, BudgetPartKind, LayoutField, LayoutFieldKind};
use crate::primitives::{NonEmptyText, OneOrMore};

// --- 8. `Layout` -----------------------------------------------------------------

pub(super) fn layout_markup(
    caption: Option<&NonEmptyText>,
    unit: &NonEmptyText,
    fields: &OneOrMore<LayoutField>,
) -> Markup {
    let fields = fields.as_slice();
    let raw_sizes: Vec<u32> = fields.iter().map(|field| field.size.get()).collect();
    let total: u64 = raw_sizes.iter().map(|&size| u64::from(size)).sum();
    let raw_offsets = raw_offsets(&raw_sizes);

    let caption_line = caption.map_or_else(
        || format!("{total}{} total", unit.as_str()),
        |caption| format!("{} \u{b7} {total}{} total", caption.as_str(), unit.as_str()),
    );

    markup! {
        div class="d layout" {
            div class="lay-bar" {
                @for (field, raw_offset) in fields.iter().zip(&raw_offsets) {
                    (layout_field(field, *raw_offset, unit))
                }
            }
            div class="d-cap" { (caption_line) }
        }
    }
}

/// `offsets[i]` = sum of `sizes[0..i]` — each field's running byte/bit offset
/// before it (`render.py` `d_layout`).
fn raw_offsets(sizes: &[u32]) -> Vec<u64> {
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut running: u64 = 0;
    for &size in sizes {
        offsets.push(running);
        running += u64::from(size);
    }
    offsets
}

fn layout_field(field: &LayoutField, raw_offset: u64, unit: &NonEmptyText) -> Markup {
    let class = format!("lay-f {}", layout_field_class(field.kind));
    markup! {
        div class=(class) style=(format!("--g:{}", field.size.get())) {
            span class="lay-n" { (field.name.as_str()) }
            span class="lay-s" {
                (raw_offset) span class="lay-sep" { "+" } (field.size.get()) (unit.as_str())
            }
            @if let Some(note) = &field.note {
                span class="note" { (note.as_str()) }
            }
        }
    }
}

const fn layout_field_class(kind: Option<LayoutFieldKind>) -> &'static str {
    match kind {
        Some(LayoutFieldKind::New) => "lay-new",
        Some(LayoutFieldKind::Reserved) => "lay-reserved",
        Some(LayoutFieldKind::Removed) => "lay-removed",
        None => "lay-normal",
    }
}

// --- 13. `Budget` ------------------------------------------------------------------

pub(super) fn budget_markup(
    budget: f64,
    unit: &NonEmptyText,
    parts: &OneOrMore<BudgetPart>,
) -> Markup {
    let parts = parts.as_slice();
    let used: f64 = parts.iter().map(|part| part.value).sum();
    // Render-time guard, never a panic (mirrors `render.py`'s `or 1`).
    let scale = budget.max(used).max(1.0);
    let slack = budget - used;

    markup! {
        div class="d budget" {
            div class="bg-bar" {
                @for part in parts {
                    (budget_segment(part, unit, scale))
                }
                @if slack > 0.0 {
                    (slack_segment(unit, slack, scale))
                }
            }
            div class="d-cap" {
                (num(used)) (unit.as_str()) " of " (num(budget)) (unit.as_str()) " budget"
            }
            @if used > budget {
                div class="bg-over" { "over budget by " (num(used - budget)) (unit.as_str()) }
            }
        }
    }
}

fn budget_segment(part: &BudgetPart, unit: &NonEmptyText, scale: f64) -> Markup {
    let class = format!("bg-seg {}", budget_part_class(part.kind));
    let value = num(part.value.max(0.0));
    markup! {
        div
            class=(class)
            style=(format!("--w:{:.4}", part.value.max(0.0) * 100.0 / scale))
            title=(format!("{} \u{2014} {value}{}", part.name.as_str(), unit.as_str()))
        {
            span class="bg-n" { (part.name.as_str()) }
            span class="bg-v" { (value) (unit.as_str()) }
        }
    }
}

fn slack_segment(unit: &NonEmptyText, slack: f64, scale: f64) -> Markup {
    markup! {
        div class="bg-seg bg-slack" style=(format!("--w:{:.4}", slack * 100.0 / scale)) {
            span class="bg-n" { "slack" }
            span class="bg-v" { (num(slack)) (unit.as_str()) }
        }
    }
}

const fn budget_part_class(kind: Option<BudgetPartKind>) -> &'static str {
    match kind {
        Some(BudgetPartKind::New) => "bg-new",
        Some(BudgetPartKind::Risk) => "bg-risk",
        None => "bg-normal",
    }
}

/// Format a budget quantity like render.py's `{:g}` — drop a trailing `.0` so a
/// whole number reads `40ms`, not `40.0ms`. Rust's `f64` `Display` already does
/// this; the helper names the intent and keeps the call sites terse.
fn num(value: f64) -> String {
    format!("{value}")
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
    fn test_layout_rejects_a_zero_size_field() {
        let json = serde_json::json!({
            "kind": "layout", "caption": null, "unit": "B",
            "fields": [{"name": "header", "size": 0, "kind": null, "note": null}]
        });
        serde_json::from_value::<Diagram>(json).expect_err("size 0 must be rejected");
    }

    #[test]
    fn test_layout_field_grow_is_proportional_to_its_size() {
        let html = render(serde_json::json!({
            "kind": "layout", "caption": null, "unit": "B",
            "fields": [
                {"name": "payload", "size": 3, "kind": null, "note": null},
                {"name": "crc", "size": 1, "kind": null, "note": null}
            ]
        }));
        // The bar segments carry flex-grow = size, so 3:1 widths.
        assert!(html.contains("--g:3"));
        assert!(html.contains("--g:1"));
        assert!(html.contains("class=\"lay-bar\""));
    }

    #[test]
    fn test_layout_renders_the_total_caption() {
        let html = render(serde_json::json!({
            "kind": "layout", "caption": "wire frame", "unit": "B",
            "fields": [{"name": "header", "size": 4, "kind": "new", "note": null}]
        }));
        assert!(html.contains("lay-new"));
        assert!(html.contains("4B total"));
        assert!(html.contains("wire frame"));
    }

    #[test]
    fn test_budget_negative_value_clamps_to_zero_width_without_panicking() {
        let html = render(serde_json::json!({
            "kind": "budget", "budget": 100.0, "unit": "ms",
            "parts": [{"name": "refund", "value": -20.0, "kind": null}]
        }));
        assert!(html.contains("--w:0.0000"));
    }

    #[test]
    fn test_budget_always_renders_the_used_of_budget_caption() {
        let html = render(serde_json::json!({
            "kind": "budget", "budget": 100.0, "unit": "ms",
            "parts": [{"name": "db", "value": 40.0, "kind": null}]
        }));
        assert!(html.contains("<div class=\"d-cap\">40ms of 100ms budget</div>"));
    }

    #[test]
    fn test_budget_over_budget_renders_the_over_caption() {
        let html = render(serde_json::json!({
            "kind": "budget", "budget": 100.0, "unit": "ms",
            "parts": [{"name": "db", "value": 140.0, "kind": null}]
        }));
        assert!(html.contains("bg-over"));
        assert!(html.contains("over budget by"));
    }

    #[test]
    fn test_budget_under_budget_renders_a_slack_segment() {
        let html = render(serde_json::json!({
            "kind": "budget", "budget": 100.0, "unit": "ms",
            "parts": [{"name": "db", "value": 40.0, "kind": null}]
        }));
        assert!(html.contains("bg-slack"));
        assert!(!html.contains("bg-over"));
    }

    #[test]
    fn test_rendering_the_same_budget_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "budget", "budget": 100.0, "unit": "ms",
            "parts": [{"name": "db", "value": 40.0, "kind": "risk"}]
        });
        assert_eq!(render(json.clone()), render(json));
    }
}
