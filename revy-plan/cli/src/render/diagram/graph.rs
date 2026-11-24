//! `revy:flow`/`revy:states`/`revy:layers`/`revy:timeline`/`revy:tree` — the 5
//! box-list diagram kinds `FP2-HTML-2` ports to render.py's own HTML/CSS
//! (`.fl-*`/`.vf-*`, `.stt-*`/`.tr-*`, `.ly-*`/`.lye-*`, `.tl-*`, `.tre-*`),
//! replacing the SVG-geometry drawers R2a originally gave them (see this
//! file's own git history for the deleted `NodeBox`/`Edge`-based versions).
//! `Dag` (`FP2-HTML`, unchanged below) and `Seq` (`seq.rs`, unchanged) were
//! ported the same way already. Every one of these — like `seq`/`dag` before
//! them — never computes pixel geometry: render.py emits plain flex/grid HTML
//! the browser measures and wraps itself, so there is no card/row height or
//! column width left to predict or clip against. See `render.py:278-320`
//! (`flow`), `:593-620` (`states`), `:413-441` (`layers`), `:793-809`
//! (`timeline`), `:744-768` (`tree`); the matching CSS line ranges are cited
//! in `plan.css`'s own comments for each section.
//!
//! Colour vars are adapted to plan.css's existing tokens, the same mapping
//! `FP2-HTML` already established for `dag`/`seq`: render.py's `--c-req`/
//! `--c-req-bg` → `--error`/`--error-bg`, `--c-header`/`--c-header-bg` →
//! `--reference`/`--reference-bg`, `--c-path`/`--c-path-bg` → `--context`/
//! `--context-bg`. This task additionally needs `--c-body`/`--c-body-bg` (and
//! the literal `#067647` render.py hardcodes outside any var in a few rules,
//! e.g. `.vf-box.terminal`/`.chip.new` — the same colour) → `--new`/
//! `--new-bg`, a token plan.css already defined but never used until now,
//! evidently reserved for exactly this green "added/new/terminal" semantic —
//! distinct from the SVG-era `--pass` every other `dg-kind-new` rule still
//! uses. render.py's sixth, unlabelled tier colour (`--c-query`, teal,
//! `layers`' `--t:2` slot) has no plan.css analogue; mapped to the existing
//! `--accent` token rather than inventing a new one.

use maud::{Markup, html as markup};

use super::Icon;
use crate::model::diagram::{
    Crossing, FlowOrientation, FlowStep, LayerNode, LayerNodeKind, MrKind, Outcome, OutcomeKind,
    State, StateKind, Tier, TimelineStep, TimelineStepKind, Transition, TransitionKind,
    TreeChangeKind, TreeEntry, VerticalStep, Wave, WaveMr,
};
use crate::primitives::{NonEmptyText, OneOrMore};

// --- 1. `Layers` -------------------------------------------------------------------

/// render.py's `d_layers` (`render.py:413-441`): a `.d.layers` flex column of
/// `.ly` tier rows (a `.ly-down` arrow connector between them), plus an
/// optional `.ly-edges` grid of `.lye` rows below — a 4-column CSS grid
/// (`from`/`relation`/`to`/`note`) each `.lye` joins via `display:contents`,
/// so every row's columns line up down the page. Unlike (unchanged) `dag`,
/// which still shares the pre-existing `crossing_row`/`.dg-crossings` `<ul>`
/// (see that section's own doc comment), `layers`' edges now use render.py's
/// own markup verbatim.
pub(super) fn layers_markup(layers: &OneOrMore<Tier>, edges: &[Crossing]) -> Markup {
    let tiers = layers.as_slice();
    markup! {
        div class="d layers" {
            @for (index, tier) in tiers.iter().enumerate() {
                @if index > 0 {
                    div class="ly-down" {}
                }
                (tier_row(index, tier))
            }
        }
        @if !edges.is_empty() {
            div class="ly-edges" {
                @for edge in edges {
                    (layer_edge_row(edge))
                }
            }
        }
    }
}

/// One `.ly` tier row — render.py:424-428. `--t:{i % 6}` cycles render.py's
/// own 6-colour tier palette (`plan.css`'s `.ly[style*="--t:N"]` rules).
fn tier_row(index: usize, tier: &Tier) -> Markup {
    let style = format!("--t:{}", index % 6);
    markup! {
        div class="ly" style=(style) {
            div class="ly-name" { (tier.name.as_str()) }
            div class="ly-nodes" {
                @for node in tier.nodes.as_slice() {
                    (layer_node_chip(node))
                }
            }
        }
    }
}

fn layer_node_chip(node: &LayerNode) -> Markup {
    chip_markup(node.name.as_str(), layer_node_chip_kind(node.kind))
}

const fn layer_node_chip_kind(kind: Option<LayerNodeKind>) -> &'static str {
    match kind {
        Some(LayerNodeKind::New) => "new",
        Some(LayerNodeKind::External) => "external",
        None => "",
    }
}

/// One `.lye` edge row — render.py:431-439. `.lye-a`/`.lye-r`/`.lye-b`/
/// `.lye-n` are always all 4 present (never conditionally omitted), even
/// when `note` is empty — `.ly-edges`' `display:contents` grid technique
/// depends on every row contributing exactly 4 cells, or a later row's cells
/// would drift into the wrong column.
fn layer_edge_row(edge: &Crossing) -> Markup {
    let class = if edge.absent { "lye none" } else { "lye" };
    markup! {
        div class=(class) {
            div class="lye-a" { (chip_markup(edge.from.as_str(), "")) }
            div class="lye-r" {
                @if let Some(label) = &edge.label { (label.as_str()) } @else { "\u{2192}" }
            }
            div class="lye-b" { (chip_markup(edge.to.as_str(), "")) }
            div class="lye-n" {
                @if let Some(note) = &edge.note { (note.as_str()) }
            }
        }
    }
}

/// render.py's `chip()`/`icon()` (`render.py:258-275`) ported directly: a
/// `<span class="chip {kind}">` with an optional leading glyph for the `new`/
/// `external` kinds (`CHIP_ICONS`), verbatim down to the trailing space
/// `chip "" ""` leaves in the class attribute when `kind` is empty.
fn chip_markup(text: &str, kind: &str) -> Markup {
    let class = format!("chip {kind}");
    markup! {
        span class=(class) {
            @if kind == "new" {
                (Icon::Plus.markup())
            } @else if kind == "external" {
                (Icon::Cloud.markup())
            }
            (text)
        }
    }
}

/// One `Crossing` row — the pre-existing `<ul>`/`<li>` markup, now only used
/// by (unchanged) `dag`'s edges list; `layers` moved to `layer_edge_row`
/// above. An `absent` crossing renders muted with no arrowhead glyph.
fn crossing_row(edge: &Crossing) -> Markup {
    let row_class = if edge.absent {
        "dg-crossing dg-crossing-absent"
    } else {
        "dg-crossing"
    };
    markup! {
        li class=(row_class) {
            span class="dg-chip" { (edge.from.as_str()) }
            span class="dg-crossing-glyph" {
                @if edge.absent { "⋯" } @else { "→" }
            }
            span class="dg-chip" { (edge.to.as_str()) }
            @if let Some(label) = &edge.label {
                span class="dg-crossing-label" { (label.as_str()) }
            }
            @if let Some(note) = &edge.note {
                span class="dg-crossing-note" { (note.as_str()) }
            }
        }
    }
}

// --- 2. `Flow` -----------------------------------------------------------------------

/// render.py's `d_flow`/`_flow_vertical` (`render.py:278-320`): a horizontal
/// pipeline (`.d.flow`, `.fl-node` boxes joined by `.fl-arrow`s) or a vertical
/// path with every early exit drawn beside it (`.d.vflow`, `.vf-step`s with
/// an optional `.vf-outs` list, `.vf-down` connectors between steps).
pub(super) fn flow_markup(flow: &FlowOrientation) -> Markup {
    match flow {
        FlowOrientation::Horizontal { steps } => flow_horizontal_markup(steps),
        FlowOrientation::Vertical { steps } => flow_vertical_markup(steps),
    }
}

/// render.py:286-291 — `'<div class="fl-arrow">▶</div>'.join(cells)`: an
/// arrow between every pair of nodes, none trailing the last.
fn flow_horizontal_markup(steps: &OneOrMore<FlowStep>) -> Markup {
    let steps = steps.as_slice();
    markup! {
        div class="d flow" {
            @for (index, step) in steps.iter().enumerate() {
                div class="fl-node" {
                    (step.name.as_str())
                    @if let Some(note) = &step.note {
                        span class="note" { (note.as_str()) }
                    }
                }
                @if index + 1 < steps.len() {
                    div class="fl-arrow" { "\u{25b6}" }
                }
            }
        }
    }
}

/// render.py:294-320 — a `.vf-down` connector before every step but the
/// first (`if i:` in the Python loop).
fn flow_vertical_markup(steps: &OneOrMore<VerticalStep>) -> Markup {
    markup! {
        div class="d vflow" {
            @for (index, step) in steps.as_slice().iter().enumerate() {
                @if index > 0 {
                    div class="vf-down" {}
                }
                (vertical_step(step))
            }
        }
    }
}

/// render.py's own schema defaults an outcome's absent `kind` to `"error"`
/// and lets any string through as a box class (`render.py:306,308`); this
/// crate's [`VerticalStep`] instead only carries `terminal: bool` (`kind`
/// itself is dropped — see `model/diagram.rs`'s own §0.5 note) and every
/// [`Outcome::kind`] is a required, closed [`OutcomeKind`] — so `terminal`
/// is the only box-class token left to port (matches render.py's own
/// `.vf-box.terminal` rule, the one class token its schema-free `kind` value
/// could realistically carry in practice).
fn vertical_step(step: &VerticalStep) -> Markup {
    let box_class = if step.terminal {
        "vf-box terminal"
    } else {
        "vf-box"
    };
    markup! {
        div class="vf-step" {
            div class=(box_class) {
                (step.name.as_str())
                @if let Some(note) = &step.note {
                    div class="note" { (note.as_str()) }
                }
            }
            @if !step.outcomes.is_empty() {
                div class="vf-outs" {
                    @for outcome in &step.outcomes {
                        (outcome_row(outcome))
                    }
                }
            }
        }
    }
}

/// One `.vf-out` — render.py:307-313. `when`/`then` are two separate spans
/// (`.vf-when`/`.vf-then`), not one joined string: the `·` between them is
/// CSS-generated (`.vf-when::after`), not embedded text.
fn outcome_row(outcome: &Outcome) -> Markup {
    let (class, icon) = match outcome.kind {
        OutcomeKind::Error => ("vf-out error", Icon::Alert),
        OutcomeKind::Exit => ("vf-out exit", Icon::Exit),
    };
    markup! {
        div class=(class) {
            (icon.markup())
            span class="vf-when" { (outcome.when.as_str()) }
            span class="vf-then" { (outcome.then.as_str()) }
        }
    }
}

// --- 6. `Dag` (unchanged, `FP2-HTML`) -------------------------------------------------
//
// FP2-HTML: `render.py`'s `d_dag` (`render.py:444-505`) is plain HTML — a
// `.wv-stage` flex column of stages, each a `.wv-mrs` flex-wrap row of MR
// cards (`.wv-*`, ported verbatim to `plan.css`) — so the browser measures
// and wraps every card/gate line itself; there is no card/gate height to
// predict or canvas to size (the pre-`FP2-HTML` `<svg>` version, deleted
// here, existed only to reproduce that for a fixed-geometry canvas). The
// `edges`/`caption` tail (`crossing_row`, `dg-caption`) is unchanged: it
// already rendered as plain HTML and stays out of this port's scope (brief
// §"What to change"). As of `FP2-HTML-2`, `crossing_row`/`.dg-crossings` is
// `dag`-only — `layers` (above) moved to render.py's own `.lye-*` markup.

pub(super) fn dag_markup(
    caption: Option<&NonEmptyText>,
    waves: &OneOrMore<Wave>,
    edges: &[Crossing],
) -> Markup {
    markup! {
        div class="d dag" {
            @for (index, wave) in waves.as_slice().iter().enumerate() {
                (wave_stage(index + 1, wave))
            }
        }
        @if !edges.is_empty() {
            ul class="dg-crossings" {
                @for edge in edges {
                    (crossing_row(edge))
                }
            }
        }
        @if let Some(caption) = caption {
            p class="dg-caption" { (caption.as_str()) }
        }
    }
}

/// One `.wv-stage` — render.py:481-489's dot + heading + MR cards + optional
/// gate line, `ordinal` being the 1-based stage number render.py's own
/// `enumerate(..., start=1)` produces (render.py:453).
fn wave_stage(ordinal: usize, wave: &Wave) -> Markup {
    let mrs = wave.mrs.as_slice();
    markup! {
        div class="wv-stage" {
            span class="wv-dot" {}
            div class="wv-body" {
                div class="wv-h" {
                    (wave_label(ordinal, wave.name.as_ref()))
                    span class="wv-par" { (mrs.len()) " in parallel" }
                }
                div class="wv-mrs" {
                    @for mr in mrs {
                        (wave_mr_card(mr))
                    }
                }
                @if let Some(gate) = &wave.gate {
                    div class="wv-gate" {
                        (Icon::Alert.markup())
                        "before the next stage: " (gate.as_str())
                    }
                }
            }
        }
    }
}

/// render.py:480's `f"stage {i}" + (f" · {esc(name)}" if name else "")`.
fn wave_label(ordinal: usize, name: Option<&NonEmptyText>) -> Markup {
    markup! {
        "stage " (ordinal)
        @if let Some(name) = name {
            " \u{b7} " (name.as_str())
        }
    }
}

/// One `.mr` card — render.py:460-479.
fn wave_mr_card(mr: &WaveMr) -> Markup {
    let class = format!("mr {}", mr_kind_class(mr.kind));
    markup! {
        div class=(class) {
            a class="mr-id" href=(format!("#mr-{}", mr.id.as_str())) { "MR:" (mr.id.as_str()) }
            span class="mr-t" { (mr.title.as_str()) }
            @if let Some(note) = &mr.note {
                span class="note" { (note.as_str()) }
            }
            @if mr.flag.is_some() || mr.run.is_some() {
                div class="mr-tags" {
                    @if let Some(flag) = &mr.flag {
                        span class="mr-tag mr-flag" { (Icon::Flag.markup()) (flag.as_str()) }
                    }
                    @if let Some(run) = &mr.run {
                        span class="mr-tag mr-run" { (Icon::Exit.markup()) (run.as_str()) }
                    }
                }
            }
        }
    }
}

/// render.py:462's `esc(m.get("kind","") or "normal")` — `kind` absent maps
/// to the literal `normal` token, not an empty class.
const fn mr_kind_class(kind: Option<MrKind>) -> &'static str {
    match kind {
        Some(MrKind::Migration) => "mr-k-migration",
        Some(MrKind::Oneway) => "mr-k-oneway",
        Some(MrKind::Scaffolding) => "mr-k-scaffolding",
        None => "mr-k-normal",
    }
}

// --- 7. `States` -----------------------------------------------------------------------

/// render.py's `d_states` (`render.py:593-620`): a `.stt-set` flex-wrap row
/// of state cards, a `.tr-set` flex column of transition rows. Unlike old
/// Python, this crate's `states` is not deduplicated by name (`meta[name] =
/// st` in render.py silently keeps only the last of a repeated name) — the
/// model doesn't validate uniqueness either (see `model/diagram.rs`'s own
/// `Transition` doc comment on `from`/`to` not being cross-checked), so every
/// authored [`State`] renders its own card.
pub(super) fn states_markup(states: &OneOrMore<State>, transitions: &[Transition]) -> Markup {
    markup! {
        div class="d states" {
            div class="stt-set" {
                @for state in states.as_slice() {
                    (state_card(state))
                }
            }
            div class="tr-set" {
                @for transition in transitions {
                    (transition_row(transition))
                }
            }
        }
    }
}

/// One `.stt` card — render.py:600-606. `kind` absent maps to the literal
/// `normal` token (render.py's `esc(s.get("kind","") or "normal")`).
fn state_card(state: &State) -> Markup {
    let class = format!("stt stt-{}", state_kind_token(state.kind));
    markup! {
        div class=(class) {
            span class="stt-n" { (state.name.as_str()) }
            @if let Some(note) = &state.note {
                span class="note" { (note.as_str()) }
            }
        }
    }
}

const fn state_kind_token(kind: Option<StateKind>) -> &'static str {
    match kind {
        Some(StateKind::Initial) => "initial",
        Some(StateKind::Terminal) => "terminal",
        Some(StateKind::Error) => "error",
        Some(StateKind::New) => "new",
        None => "normal",
    }
}

/// One `.tr-row` — render.py:607-616. `.tr-on` always renders (even empty,
/// when `on` is absent) — render.py's own `esc(t.get("on",""))` has no `if`
/// guard around it, unlike the trailing `.note` span.
fn transition_row(transition: &Transition) -> Markup {
    let to_class = format!("tr-t tr-{}", transition_kind_token(transition.kind));
    markup! {
        div class="tr-row" {
            span class="tr-f" { (transition.from.as_str()) }
            span class="tr-on" {
                @if let Some(on) = &transition.on { (on.as_str()) }
            }
            span class="tr-a" { "\u{25b6}" }
            span class=(to_class) { (transition.to.as_str()) }
            @if let Some(note) = &transition.note {
                span class="note" { (note.as_str()) }
            }
        }
    }
}

const fn transition_kind_token(kind: Option<TransitionKind>) -> &'static str {
    match kind {
        Some(TransitionKind::Error) => "error",
        Some(TransitionKind::New) => "new",
        None => "normal",
    }
}

// --- 9. `Timeline` ---------------------------------------------------------------------

/// render.py's `d_timeline` (`render.py:793-809`): a `.d.timeline` flex
/// column of `.tl-step` rows, each a dot plus a body (name, optional wait
/// chip, note, rollback line) — the rail between dots is a CSS `::before` on
/// every step but the last, not a drawn line.
pub(super) fn timeline_markup(steps: &OneOrMore<TimelineStep>) -> Markup {
    markup! {
        div class="d timeline" {
            @for step in steps.as_slice() {
                (timeline_row(step))
            }
        }
    }
}

/// One `.tl-step` — render.py:796-806. `kind` absent maps to the literal
/// `normal` token.
fn timeline_row(step: &TimelineStep) -> Markup {
    let class = format!("tl-step tl-{}", timeline_kind_token(step.kind));
    markup! {
        div class=(class) {
            span class="tl-dot" {}
            div class="tl-body" {
                span class="tl-n" { (step.name.as_str()) }
                @if let Some(wait) = &step.wait {
                    span class="tl-w" { (wait.as_str()) }
                }
                @if let Some(note) = &step.note {
                    div class="note" { (note.as_str()) }
                }
                @if let Some(rollback) = &step.rollback {
                    div class="tl-rb" { "rollback: " (rollback.as_str()) }
                }
            }
        }
    }
}

const fn timeline_kind_token(kind: Option<TimelineStepKind>) -> &'static str {
    match kind {
        Some(TimelineStepKind::Gate) => "gate",
        Some(TimelineStepKind::Oneway) => "oneway",
        None => "normal",
    }
}

// --- 10. `Tree` ------------------------------------------------------------------------

/// A `--d` custom property caps a row's indent (render.py:763's own
/// `min(depth, 8)`) so one absurdly deep path cannot push a row off the page.
const TREE_MAX_DEPTH: usize = 8;

/// render.py's `d_tree` (`render.py:744-768`): one `.tre-row` per entry,
/// indented by depth via `--d` (`plan.css`'s `calc(var(--d) * 1.4rem)`).
/// Depth is normalized against the shallowest entry in the diagram
/// (`base_depth`, mirroring render.py's own `base_depth` subtraction at
/// `render.py:747-751,756`), not [`crate::primitives::RepoRelPath`]'s raw
/// absolute slash count off the repo root (A6). An earlier version of this
/// comment framed the raw-depth choice as "deliberate" — that didn't hold up:
/// a real authored tree's entries share a long common prefix (the crate or
/// module the plan is touching), so their absolute depth clusters near
/// [`TREE_MAX_DEPTH`] and the diagram reads flat. Subtracting the shallowest
/// entry's depth restores the meaningful 1-2 level structural difference the
/// diagram is meant to show.
pub(super) fn tree_markup(entries: &OneOrMore<TreeEntry>) -> Markup {
    let base_depth = entries
        .as_slice()
        .iter()
        .map(|entry| entry.path.as_str().matches('/').count())
        .min()
        .unwrap_or(0);
    markup! {
        div class="d tree" {
            @for entry in entries.as_slice() {
                (tree_row(entry, base_depth))
            }
        }
    }
}

fn tree_row(entry: &TreeEntry, base_depth: usize) -> Markup {
    let depth = entry
        .path
        .as_str()
        .matches('/')
        .count()
        .saturating_sub(base_depth)
        .min(TREE_MAX_DEPTH);
    let name = entry
        .path
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or(entry.path.as_str());
    let kind = tree_kind_token(entry.change);
    let name_class = format!("tre-n tre-{kind}");
    let style = format!("--d:{depth}");

    markup! {
        div class="tre-row" style=(style) {
            code class=(name_class) { (name) }
            @if kind != "same" {
                span class=(format!("tre-b tre-{kind}")) { (kind) }
            }
            @if let Some(note) = &entry.note {
                span class="note" { (note.as_str()) }
            }
        }
    }
}

/// render.py:758's `esc(e.get("change","") or "same")` — `change` absent
/// maps to the literal `same` token, which also suppresses the badge
/// (render.py:759-761's `if kind != "same"`).
const fn tree_kind_token(kind: Option<TreeChangeKind>) -> &'static str {
    match kind {
        Some(TreeChangeKind::Added) => "added",
        Some(TreeChangeKind::Modified) => "modified",
        Some(TreeChangeKind::Removed) => "removed",
        None => "same",
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

    // --- Layers: `.d layers` wrapper, `.lye`/`.lye none` class, chip kinds -------

    #[test]
    fn test_layers_renders_the_d_layers_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": null}]}],
            "edges": []
        }));

        assert!(html.contains("class=\"d layers\""));
        assert!(
            !html.contains("<svg"),
            "layers must no longer render an <svg> canvas: {html}"
        );
    }

    #[test]
    fn test_layers_absent_edge_renders_the_lye_none_class_present_edge_renders_lye_only() {
        let html = render(serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": null}]}],
            "edges": [
                {"from": "api", "to": "db", "label": null, "note": null, "absent": true},
                {"from": "api", "to": "cache", "label": null, "note": null, "absent": false}
            ]
        }));

        assert!(html.contains("class=\"lye none\""));
        assert!(html.contains("class=\"lye\""));
    }

    #[test]
    fn test_layers_edge_note_always_renders_the_lye_n_div_even_when_empty() {
        // `.ly-edges`' `display:contents` grid needs exactly 4 cells per row
        // regardless of content — an omitted empty `.lye-n` would misalign
        // every subsequent row into the wrong column.
        let html = render(serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": null}]}],
            "edges": [{"from": "api", "to": "db", "label": null, "note": null, "absent": false}]
        }));

        assert!(html.contains("<div class=\"lye-n\"></div>"));
    }

    #[test]
    fn test_layers_new_kind_node_renders_the_chip_new_class_with_a_glyph() {
        let html = render(serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": "new"}]}],
            "edges": []
        }));

        assert!(html.contains("class=\"chip new\""));
        assert!(
            html.contains("<svg"),
            "the `new` chip must carry a glyph: {html}"
        );
    }

    #[test]
    fn test_layers_edge_endpoint_chips_carry_no_kind_class_or_glyph() {
        let html = render(serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": null}]}],
            "edges": [{"from": "api", "to": "db", "label": null, "note": null, "absent": false}]
        }));

        assert!(html.contains("class=\"chip \""));
    }

    // --- Flow: terminal border class, outcome kind colours, two separate spans --

    #[test]
    fn test_flow_horizontal_renders_the_d_flow_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "horizontal",
                "steps": [{"name": "Receive request", "note": null}]
            }
        }));

        assert!(html.contains("class=\"d flow\""));
        assert!(
            !html.contains("<svg"),
            "flow must no longer render an <svg> canvas: {html}"
        );
    }

    #[test]
    fn test_flow_horizontal_arrow_joins_nodes_but_does_not_trail_the_last() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "horizontal",
                "steps": [
                    {"name": "A", "note": null},
                    {"name": "B", "note": null},
                    {"name": "C", "note": null}
                ]
            }
        }));

        assert_eq!(html.matches("fl-arrow").count(), 2);
    }

    #[test]
    fn test_flow_vertical_renders_the_d_vflow_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{"name": "Validate", "note": null, "terminal": false, "outcomes": []}]
            }
        }));

        assert!(html.contains("class=\"d vflow\""));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn test_flow_vertical_terminal_step_renders_the_terminal_border_class() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{
                    "name": "Respond",
                    "note": null,
                    "terminal": true,
                    "outcomes": []
                }]
            }
        }));

        assert!(html.contains("class=\"vf-box terminal\""));
    }

    #[test]
    fn test_flow_vertical_outcome_kind_colours_the_outcome_and_splits_when_then() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{
                    "name": "Validate",
                    "note": null,
                    "terminal": false,
                    "outcomes": [
                        {"when": "invalid", "then": "400", "kind": "error"},
                        {"when": "cached", "then": "304", "kind": "exit"}
                    ]
                }]
            }
        }));

        assert!(html.contains("class=\"vf-out error\""));
        assert!(html.contains("class=\"vf-out exit\""));
        assert!(html.contains("class=\"vf-when\">invalid<"));
        assert!(html.contains("class=\"vf-then\">400<"));
    }

    #[test]
    fn test_flow_vertical_step_without_outcomes_omits_the_vf_outs_wrapper() {
        let html = render(serde_json::json!({
            "kind": "flow",
            "flow": {
                "orientation": "vertical",
                "steps": [{"name": "Respond", "note": null, "terminal": true, "outcomes": []}]
            }
        }));

        assert!(!html.contains("vf-outs"));
    }

    // --- States: kind-token classes, `normal` default, always-present `.tr-on` --

    #[test]
    fn test_states_renders_the_d_states_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "states",
            "states": [{"name": "Idle", "kind": null, "note": null}],
            "transitions": []
        }));

        assert!(html.contains("class=\"d states\""));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn test_states_kind_absent_defaults_to_the_normal_token() {
        let html = render(serde_json::json!({
            "kind": "states",
            "states": [{"name": "Idle", "kind": null, "note": null}],
            "transitions": []
        }));

        assert!(html.contains("class=\"stt stt-normal\""));
    }

    #[test]
    fn test_states_error_transition_renders_the_tr_error_class() {
        let html = render(serde_json::json!({
            "kind": "states",
            "states": [
                {"name": "Idle", "kind": "initial", "note": null},
                {"name": "Failed", "kind": "error", "note": null}
            ],
            "transitions": [
                {"from": "Idle", "to": "Failed", "on": "timeout", "kind": "error", "note": null}
            ]
        }));

        assert!(html.contains("class=\"tr-t tr-error\""));
    }

    #[test]
    fn test_states_transition_without_on_still_renders_the_tr_on_span() {
        let html = render(serde_json::json!({
            "kind": "states",
            "states": [{"name": "Idle", "kind": null, "note": null}],
            "transitions": [
                {"from": "Idle", "to": "Idle", "on": null, "kind": null, "note": null}
            ]
        }));

        assert!(html.contains("<span class=\"tr-on\"></span>"));
    }

    // --- Timeline: rollback line present/absent, kind-token classes -------------

    #[test]
    fn test_timeline_renders_the_d_timeline_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "timeline",
            "steps": [{"name": "Deploy", "kind": null, "wait": null, "rollback": null, "note": null}]
        }));

        assert!(html.contains("class=\"d timeline\""));
        assert!(!html.contains("<svg"));
    }

    #[test]
    fn test_timeline_step_with_rollback_renders_the_tl_rb_line() {
        let html = render(serde_json::json!({
            "kind": "timeline",
            "steps": [{
                "name": "Flip flag",
                "kind": "gate",
                "wait": "10m",
                "rollback": "Flip the flag back off.",
                "note": null
            }]
        }));

        assert!(html.contains("class=\"tl-rb\""));
        assert!(html.contains("Flip the flag back off."));
        assert!(html.contains("class=\"tl-step tl-gate\""));
    }

    #[test]
    fn test_timeline_step_without_rollback_omits_the_tl_rb_line() {
        let html = render(serde_json::json!({
            "kind": "timeline",
            "steps": [{
                "name": "Flip flag",
                "kind": "gate",
                "wait": null,
                "rollback": null,
                "note": null
            }]
        }));

        assert!(!html.contains("tl-rb"));
    }

    // --- Dag: `Slug` rejection + oneway border class -----------------------------

    #[test]
    fn test_dag_rejects_a_wave_mr_id_that_fails_slug_parsing() {
        let json = serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{"id": "mr_1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("an id containing `_` must fail `Slug` parsing");
    }

    #[test]
    fn test_dag_oneway_mr_renders_the_oneway_border_class() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": "release plan",
            "waves": [{
                "name": "wave-1",
                "mrs": [{"id": "mr-1", "title": "Backfill", "kind": "oneway", "flag": null, "run": null, "note": null}],
                "gate": "backfill must finish"
            }],
            "edges": []
        }));

        assert!(html.contains("mr-k-oneway"));
        assert!(html.contains("wv-gate"));
        assert!(html.contains("release plan"));
    }

    #[test]
    fn test_dag_normal_mr_kind_defaults_to_the_mr_k_normal_class() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("class=\"mr mr-k-normal\""));
    }

    #[test]
    fn test_dag_mr_tags_render_the_flag_and_run_chips() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{
                    "id": "mr-1", "title": "Add queue", "kind": null,
                    "flag": "queue_v2", "run": "cargo test", "note": null
                }],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("class=\"mr-tag mr-flag\""));
        assert!(html.contains("queue_v2"));
        assert!(html.contains("class=\"mr-tag mr-run\""));
        assert!(html.contains("cargo test"));
    }

    #[test]
    fn test_dag_wave_label_includes_the_ordinal_and_name() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": "the blocking mechanism",
                "mrs": [{"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("stage 1 \u{b7} the blocking mechanism"));
    }

    #[test]
    fn test_dag_wave_label_omits_the_dot_when_name_is_absent() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("stage 1<span"));
        assert!(!html.contains('\u{b7}'));
    }

    #[test]
    fn test_dag_wv_par_reflects_the_number_of_mrs_in_the_wave() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [
                    {"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null},
                    {"id": "mr-2", "title": "Add consumer", "kind": null, "flag": null, "run": null, "note": null}
                ],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("class=\"wv-par\">2 in parallel"));
    }

    #[test]
    fn test_rendering_the_same_dag_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "dag",
            "caption": "release plan",
            "waves": [{
                "name": "wave-1",
                "mrs": [{"id": "mr-1", "title": "Backfill", "kind": "oneway", "flag": null, "run": null, "note": null}],
                "gate": "backfill must finish"
            }],
            "edges": []
        });

        assert_eq!(render(json.clone()), render(json));
    }

    #[test]
    fn test_dag_renders_the_d_dag_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{"id": "mr-1", "title": "Add queue", "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains("class=\"d dag\""));
        assert!(
            !html.contains("<svg"),
            "dag must no longer render an <svg> canvas: {html}"
        );
    }

    #[test]
    fn test_dag_long_mr_title_renders_intact_not_truncated() {
        // render.py never truncates a card's title (the browser wraps it via
        // CSS `overflow-wrap`, plan.css's `.dg-node-sub`-era clamp no longer
        // applies to `.mr-t`) — this drawer no longer predicts a card height
        // to avoid clipping it, because there is no fixed height left to
        // clip against.
        let title = "BLPOP blocks until a value arrives, its timeout expires, or its client leaves";
        let html = render(serde_json::json!({
            "kind": "dag",
            "caption": null,
            "waves": [{
                "name": null,
                "mrs": [{"id": "mr-1", "title": title, "kind": null, "flag": null, "run": null, "note": null}],
                "gate": null
            }],
            "edges": []
        }));

        assert!(html.contains(title));
    }

    // --- Tree: trailing-slash rejection + depth-proportional `--d` --------------

    #[test]
    fn test_tree_rejects_a_path_ending_in_a_slash() {
        let json = serde_json::json!({
            "kind": "tree",
            "entries": [{"path": "src/handlers/", "change": null, "note": null}]
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("a trailing slash must be rejected by `RepoRelPath`");
    }

    /// A6: depth is normalized against the shallowest entry (`base_depth`),
    /// not the raw absolute slash count — these two entries differ by one
    /// slash (1 vs. 2), so after subtracting the shallower entry's own depth
    /// (1) they land at `--d:0` and `--d:1`, not the pre-A6 `--d:1`/`--d:2`.
    #[test]
    fn test_tree_second_entry_renders_one_depth_deeper_than_the_first_after_base_depth_normalization()
     {
        let html = render(serde_json::json!({
            "kind": "tree",
            "entries": [
                {"path": "src/lib.rs", "change": null, "note": null},
                {"path": "crates/revy-plan/mod.rs", "change": null, "note": null}
            ]
        }));

        assert!(html.contains("style=\"--d:0\""));
        assert!(html.contains("style=\"--d:1\""));
    }

    /// A6: entries that share a literal common prefix (here `crates/revy-plan/src/render/`)
    /// normalize the shallowest to indent 0 rather than clustering near
    /// `TREE_MAX_DEPTH` at their raw absolute repo-relative depth.
    #[test]
    fn test_tree_entries_sharing_a_prefix_normalize_the_shallowest_to_indent_zero() {
        let html = render(serde_json::json!({
            "kind": "tree",
            "entries": [
                {"path": "crates/revy-plan/src/render/plan.rs", "change": null, "note": null},
                {"path": "crates/revy-plan/src/render/diagram/graph.rs", "change": null, "note": null}
            ]
        }));

        assert!(html.contains("style=\"--d:0\""));
        assert!(html.contains("style=\"--d:1\""));
    }

    #[test]
    fn test_tree_renders_the_d_tree_wrapper_not_an_svg() {
        let html = render(serde_json::json!({
            "kind": "tree",
            "entries": [{"path": "src/lib.rs", "change": "added", "note": null}]
        }));

        assert!(html.contains("class=\"d tree\""));
        assert!(html.contains("tre-b tre-added"));
        assert!(
            !html.contains("<svg"),
            "tree must no longer render an <svg> canvas: {html}"
        );
    }

    #[test]
    fn test_tree_unchanged_entry_omits_the_badge() {
        let html = render(serde_json::json!({
            "kind": "tree",
            "entries": [{"path": "src/lib.rs", "change": null, "note": null}]
        }));

        assert!(!html.contains("tre-b"));
        assert!(html.contains("tre-n tre-same"));
    }

    #[test]
    fn test_rendering_the_same_tree_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "tree",
            "entries": [
                {"path": "src/lib.rs", "change": "added", "note": null},
                {"path": "crates/revy-plan/mod.rs", "change": null, "note": null}
            ]
        });

        assert_eq!(render(json.clone()), render(json));
    }

    // --- Determinism across repeated renders -------------------------------------

    #[test]
    fn test_rendering_the_same_diagram_twice_is_byte_identical() {
        let json = serde_json::json!({
            "kind": "layers",
            "layers": [{"name": "Service", "nodes": [{"name": "api", "kind": "new"}]}],
            "edges": []
        });

        assert_eq!(render(json.clone()), render(json));
    }
}
