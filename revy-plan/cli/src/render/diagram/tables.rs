//! `revy:er`/`revy:endpoint`/`revy:matrix`/`revy:compare`/`revy:stack` —
//! `FP2-HTML-3` ports these 5 to render.py's own HTML/CSS (`.er-*`, `.ep-*`,
//! `.mx`/`.mx-*`, `.compare`/`.cmp-*`, `.stack`/`.stack-row`/`.mark-*`),
//! replacing the `dg-er`/`dg-endpoint`/`dg-matrix`/`dg-compare`/`dg-stack`
//! drawers R2c originally gave them. Like `seq`/`dag`/`graph.rs`'s five
//! before them, none of these compute a position — render.py drew every one
//! as a `<table>` or a flat CSS list with zero coordinates, so the browser
//! lays them out and none of this file's drawers emit any SVG.
//! See `render.py:375-410` (`er`), `:508-590` (`endpoint`), `:771-790`
//! (`matrix`), `:812-830` (`compare`), `:202-231` (`stack`, `as_stack` — not
//! a `DIAGRAMS` entry; see `stack_markup`'s own doc comment for why its
//! wrapper class is bare `stack`, not `d stack`); the matching CSS line
//! ranges are cited in `plan.css`'s own comments for each section.
//!
//! `pk`/`fk` field badges (`er`), the auth chip (`endpoint`), and call
//! markers (`stack`) now emit render.py's own `.bdg`/`.ep-auth`/`.mark`
//! markup directly rather than the shared `dg-chip` pill — as of this task
//! the shared `Chip` primitive (`mod.rs`) has no remaining caller and is
//! deleted; `Icon` stays (`layout`/`trace`/`budget`/`dag`/`flow`/`layers`
//! still use it, and this file keeps using it for the badge/auth/relation
//! glyphs, just wrapped in render.py's own markup instead of `Chip`'s pill).

use maud::{Markup, PreEscaped, html as markup};

use super::Icon;
use crate::model::content::Language;
use crate::model::diagram::{
    Auth, Body, CompareColumn, CompareItem, CompareItemKind, Entity, EntityKind, ExampleKind,
    Field, FieldKey, HttpMethod, MatrixBody, MatrixCellKind, MatrixRow, Param, ParamLocation,
    Relation, RelationKind, Response, ResponseKind, StackFrame, StackMarker, WorkedExample,
};
use crate::primitives::{NonEmptyText, OneOrMore};
use crate::render::markdown::inline_markup;
use crate::render::plan::highlighted_code_html;

// --- 4. `Er` -----------------------------------------------------------------

/// render.py's `d_er` (`render.py:375-410`): `.er-set` a flex-wrap row of
/// `.er-e` entity cards (a `.er-h` head bar, one `.er-f` per field), an
/// optional `.er-rels` column of relation rows below.
pub(super) fn er_markup(entities: &OneOrMore<Entity>, relations: &[Relation]) -> Markup {
    markup! {
        div class="d er" {
            div class="er-set" {
                @for entity in entities {
                    (entity_card(entity))
                }
            }
            @if !relations.is_empty() {
                div class="er-rels" {
                    @for relation in relations {
                        (relation_row(relation))
                    }
                }
            }
        }
    }
}

/// render.py:392-398 — the entity's `kind` (when present) is appended as a
/// bare space-separated modifier straight onto `.er-e`'s own class
/// (`.er-e new`/`.er-e context`), not a distinct `dg-entity-new`-style class.
fn entity_card(entity: &Entity) -> Markup {
    let class = entity.kind.map_or("er-e", entity_kind_class);
    markup! {
        div class=(class) {
            div class="er-h" {
                (entity.name.as_str())
                @if let Some(kind) = entity.kind {
                    span class="er-tag" { (entity_kind_label(kind)) }
                }
            }
            @for field in entity.fields.as_slice() {
                (field_row(field))
            }
        }
    }
}

const fn entity_kind_class(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::New => "er-e new",
        EntityKind::Context => "er-e context",
    }
}

const fn entity_kind_label(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::New => "new",
        EntityKind::Context => "context",
    }
}

/// render.py:388-391 — name, then type, then the optional pk/fk badge, all
/// three siblings inside one `.er-f` row; `.er-t` is a plain `<span>` (the
/// row's own `font:...monospace` already covers it), not a `<code>`.
fn field_row(field: &Field) -> Markup {
    markup! {
        div class="er-f" {
            span class="er-n" { (field.name.as_str()) }
            span class="er-t" { (field.type_name.as_str()) }
            @if let Some(key) = &field.key {
                (field_key_badge(key))
            }
        }
    }
}

/// render.py:380-387's `.bdg pk`/`.bdg fk` badges — a distinct pill from the
/// `.chip` relation operands below, each carrying its own glyph
/// (`Icon::Key`/`Icon::Link`).
fn field_key_badge(key: &FieldKey) -> Markup {
    match key {
        FieldKey::Primary {} => markup! {
            span class="bdg pk" { (Icon::Key.markup()) "PK" }
        },
        FieldKey::Foreign { references } => markup! {
            span class="bdg fk" { (Icon::Link.markup()) (references.as_str()) }
        },
    }
}

/// render.py:399-405 — `from`/`to` render as bare `.chip` pills (render.py's
/// own module-level `chip()` called with no `cls`, i.e. no icon), joined by
/// the relation's kind label and a literal `▶`.
fn relation_row(relation: &Relation) -> Markup {
    markup! {
        div class="er-r" {
            (relation_chip(relation.from.as_str()))
            span class="er-k" { (relation_kind_label(relation.kind)) }
            "\u{25b6}"
            (relation_chip(relation.to.as_str()))
            @if let Some(label) = &relation.label {
                span class="note" { (label.as_str()) }
            }
        }
    }
}

/// render.py's `chip(text)` with no `cls` (`render.py:273-275`): a bare
/// `.chip` pill, no icon. Duplicated from `graph.rs`'s own local
/// `chip_markup` (not imported — `graph.rs` is out of this port's scope):
/// `er`'s relation operands never carry a `kind`, unlike `layers`/`dag`'s
/// edge chips, so this only needs the icon-less case.
fn relation_chip(text: &str) -> Markup {
    markup! {
        span class="chip " { (text) }
    }
}

const fn relation_kind_label(kind: RelationKind) -> &'static str {
    match kind {
        RelationKind::OneToMany => "1-n",
        RelationKind::ManyToOne => "n-1",
        RelationKind::OneToOne => "1-1",
    }
}

// --- 5. `Endpoint` -------------------------------------------------------------

/// render.py's `d_endpoint` (`render.py:508-590`): a `.d ep` (not `.d
/// endpoint` — render.py's own short class token) wrapper around an `.ep-h`
/// header bar, up to two `.ep-t` tables (params, responses), an optional
/// `.ep-req` request-body block and `.ep-warn` incompleteness note.
///
/// 7 parameters, at the default `clippy::too_many_arguments` threshold —
/// `Diagram::Endpoint`'s own field count, passed through by the dispatcher's
/// destructuring match rather than bundled into a bespoke view struct that
/// would exist for no reason but to shrink this signature.
pub(super) fn endpoint_markup(
    method: HttpMethod,
    path: &NonEmptyText,
    summary: Option<&NonEmptyText>,
    auth: &Auth,
    params: &[Param],
    request: Option<&Body>,
    responses: &OneOrMore<Response>,
) -> Markup {
    let responses = responses.as_slice();
    markup! {
        div class="d ep" {
            div class="ep-h" {
                span class=(format!("verb v-{}", verb_label(method))) { (verb_label(method)) }
                code class="ep-p" { (path.as_str()) }
                (auth_chip(auth))
            }
            @if let Some(summary) = summary {
                div class="ep-s" { (summary.as_str()) }
            }
            @if !params.is_empty() {
                table class="ep-t" {
                    thead { tr { th { "param" } th { "in" } th { "type" } th {} th { "note" } } }
                    tbody {
                        @for param in params {
                            (param_row(param))
                        }
                    }
                }
            }
            @if let Some(request) = request {
                div class="ep-req" {
                    div class="ep-lbl" { "request body" }
                    (body_block(request))
                }
            }
            table class="ep-t" {
                thead { tr { th { "status" } th { "meaning" } th { "body" } } }
                tbody {
                    @for response in responses {
                        (response_row(response))
                    }
                }
            }
            @if let Some(warning) = incompleteness_warning(responses) {
                div class="ep-warn" { (warning) }
            }
        }
    }
}

const fn verb_label(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

/// render.py:556-564 — `Open` reuses `Icon::Exit` (old Python's own choice,
/// `render.py:560`, `icon("exit", "no auth")`); a route with no auth is a
/// stated finding, not an absence, so it gets a distinct glyph from
/// `Scheme`'s lock rather than no icon at all. Only `Open` gets the extra
/// `open` class token (`.ep-auth open`); `Scheme` is the bare `.ep-auth`.
fn auth_chip(auth: &Auth) -> Markup {
    match auth {
        Auth::Open { reason } => markup! {
            span class="ep-auth open" { (Icon::Exit.markup()) (reason.as_str()) }
        },
        Auth::Scheme { name } => markup! {
            span class="ep-auth" { (Icon::Lock.markup()) (name.as_str()) }
        },
    }
}

/// render.py:512-520 — name/type render in `<code>`; `in` renders as plain
/// text inside its own `.loc loc-{in}` colour span; `required` renders a
/// `.req` badge span, not bare text.
fn param_row(param: &Param) -> Markup {
    let location = param_location_slug(param.location);
    markup! {
        tr {
            td { code { (param.name.as_str()) } }
            td { span class=(format!("loc loc-{location}")) { (location) } }
            td { code class="ty" { (param.type_name.as_str()) } }
            td { @if param.required { span class="req" { "required" } } }
            td { @if let Some(description) = &param.description { (description.as_str()) } }
        }
    }
}

const fn param_location_slug(location: ParamLocation) -> &'static str {
    match location {
        ParamLocation::Path => "path",
        ParamLocation::Query => "query",
        ParamLocation::Header => "header",
        ParamLocation::Body => "body",
    }
}

/// render.py:528-543 — the `<tr>` itself always carries the response's
/// `kind` as its class (`""` when absent, matching render.py's own
/// `esc(str(r.get("kind","") or ""))`), which is what `plan.css`'s `.ep-t
/// tr.new td, .ep-t tr.changed td` highlight rule keys off.
fn response_row(response: &Response) -> Markup {
    let row_class = response.kind.map_or("", response_kind_label);
    markup! {
        tr class=(row_class) {
            td {
                span class=(format!("st {}", status_class(response.status))) { (response.status) }
                @if let Some(kind) = response.kind {
                    span class="ep-k" { (response_kind_label(kind)) }
                }
            }
            td {
                (response.description.as_str())
                @if !response.headers.is_empty() {
                    div class="ep-hd" {
                        @for header in &response.headers {
                            code { (header.as_str()) }
                        }
                    }
                }
            }
            td { (body_block(&response.body)) }
        }
    }
}

/// `status / 100` — the hundreds digit is the exact wanted bucket (`2xx`,
/// `4xx`, …), not an approximation.
fn status_class(status: u16) -> String {
    format!("st-{}xx", status.div_euclid(100))
}

const fn response_kind_label(kind: ResponseKind) -> &'static str {
    match kind {
        ResponseKind::New => "new",
        ResponseKind::Changed => "changed",
    }
}

/// `request`/`response[].body.example` bodies reuse R1's own
/// `highlighted_code_html(Language::Json, …)` (design §4.11) — no second
/// highlighter. render.py's own `body_block` (`render.py:521-526`) shows the
/// content type as bare escaped text (no nested `<code>`); only the example
/// gets a `<pre>`/`<code>` pair.
fn body_block(body: &Body) -> Markup {
    markup! {
        @if let Some(content_type) = &body.content_type {
            span class="ep-ct" { (content_type.as_str()) }
        }
        @if let Some(example) = &body.example {
            pre class="ep-ex" {
                code { (PreEscaped(highlighted_code_html(Language::Json, example.as_str()))) }
            }
        }
    }
}

/// A render-time check, not a parse rejection (design §1.5,
/// `render.py:546-555`): fewer than 2 documented responses, or none in the
/// `4xx`/`5xx` range. Text ported verbatim, including render.py's own
/// lowercase-lead, no-trailing-period prose.
fn incompleteness_warning(responses: &[Response]) -> Option<&'static str> {
    if responses.len() < 2 {
        Some(
            "only one response documented — the call flow names every failure path, and this contract does not",
        )
    } else if responses
        .iter()
        .any(|response| matches!(response.status.div_euclid(100), 4 | 5))
    {
        None
    } else {
        Some("no failure response documented — every status the call flow names belongs here")
    }
}

// --- 11. `Matrix` --------------------------------------------------------------

/// render.py's `d_matrix` (`render.py:771-790`): `.d matrix` wraps a plain
/// `table.mx`.
pub(super) fn matrix_markup(body: &MatrixBody) -> Markup {
    let columns = body.columns.as_slice();
    markup! {
        div class="d matrix" {
            table class="mx" {
                thead {
                    tr {
                        th {}
                        @for column in columns {
                            th { (column.as_str()) }
                        }
                    }
                }
                tbody {
                    @for row in body.rows.as_slice() {
                        tr {
                            th class="mx-r" { (row.name.as_str()) }
                            @for column in columns {
                                (matrix_cell(row, column))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// render.py:776-783 — every declared column renders exactly one `<td class
/// ="mx-{kind}">`, `kind` defaulting to the literal `same` token: both an
/// explicit cell with no `kind` and a column entirely missing from
/// `row.cells` (per design §1.11, an unmarked cell is context, not an error
/// — `presentation.md:373-376`: "the cells you do not mark are the
/// argument") render `<td class="mx-same"><code></code></td>`, never a bare
/// class-less `<td></td>`.
fn matrix_cell(row: &MatrixRow, column: &NonEmptyText) -> Markup {
    let cell = row
        .cells
        .iter()
        .find(|cell| cell.column.as_str() == column.as_str());
    let kind = cell
        .and_then(|cell| cell.kind)
        .map_or("same", matrix_cell_kind_token);
    let value = cell.map_or("", |cell| cell.value.as_str());
    markup! {
        td class=(format!("mx-{kind}")) { code { (value) } }
    }
}

const fn matrix_cell_kind_token(kind: MatrixCellKind) -> &'static str {
    match kind {
        MatrixCellKind::Differs => "differs",
        MatrixCellKind::New => "new",
        MatrixCellKind::Risk => "risk",
    }
}

// --- 14. `Compare` ---------------------------------------------------------------

/// render.py's `d_compare` (`render.py:812-830`): `.compare` is a flex-wrap
/// row of `.cmp-col` columns, each a `.cmp-h` head bar plus a plain `<ul>`.
pub(super) fn compare_markup(columns: &OneOrMore<CompareColumn>) -> Markup {
    markup! {
        div class="d compare" {
            @for (index, column) in columns.as_slice().iter().enumerate() {
                (compare_column(index, column))
            }
        }
    }
}

/// render.py:826-829 — `.cmp-c{i}` is the column's 0-based *index*, not
/// anything derived from its content; `plan.css`'s `.cmp-c1 .cmp-h` rule is
/// what colours a 2-column compare's second header green (a `before`/`after`
/// convention baked into column order alone).
fn compare_column(index: usize, column: &CompareColumn) -> Markup {
    markup! {
        div class=(format!("cmp-col cmp-c{index}")) {
            div class="cmp-h" { (column.label.as_str()) }
            ul {
                @for item in column.items.as_slice() {
                    (compare_item(item))
                }
            }
        }
    }
}

/// render.py:821-824 — every `<li>` always carries a `cmp-{kind}` class,
/// `kind` defaulting to the literal `normal` token (never a bare, class-less
/// `<li>`).
fn compare_item(item: &CompareItem) -> Markup {
    let class = format!("cmp-{}", compare_item_kind_token(item.kind));
    markup! {
        li class=(class) { (item.text.as_str()) }
    }
}

const fn compare_item_kind_token(kind: Option<CompareItemKind>) -> &'static str {
    match kind {
        Some(CompareItemKind::Add) => "add",
        Some(CompareItemKind::Del) => "del",
        None => "normal",
    }
}

// --- 15. `Stack` -------------------------------------------------------------------

/// Matches `tree`'s own depth cap (`graph.rs::TREE_MAX_DEPTH`) — design §1.15
/// states the same "capped at 8" rule for `--d:{depth}`.
const STACK_MAX_DEPTH: usize = 8;

/// render.py's `as_stack` (`render.py:202-231`): the wrapper is a bare
/// `<div class="stack">`, not `<div class="d stack">` — `revy:stack` never
/// reaches the `DIAGRAMS` dispatch table at all (`render.py:871-872` calls
/// `as_stack` directly on the fence's raw text, before YAML even enters the
/// picture), so it never gets the shared `.d` class the other 14 authored
/// kinds do (confirmed: render.py's own "diagrams keep the full page width"
/// rule, `render.py:1588-1591`, lists `.body > .stack` as its own case,
/// separate from `.body > .d`).
pub(super) fn stack_markup(frames: &OneOrMore<StackFrame>) -> Markup {
    markup! {
        div class="stack" {
            @for frame in frames.as_slice() {
                (stack_frame(frame, 0))
            }
        }
    }
}

/// Recurses depth-first; `children` being recursive replaces the old text
/// grammar's indent counting entirely (design §1.15). Depth starts at `0`
/// for a root frame, matching render.py's own `expandtabs(4)//2` numbering
/// exactly (`render.py:217`): `plan.css`'s `.stack-row[style*="--d:0"]`
/// bolds the root row and suppresses its own rail segment
/// (`:not([style*="--d:0"])::before`) — starting at `1` (this drawer's own
/// pre-`FP2-HTML-3` choice) meant no row ever produced a literal `--d:0`,
/// so neither selector ever matched: every row, including the root, drew
/// with a spurious rail and without the root's bold weight.
fn stack_frame(frame: &StackFrame, depth: usize) -> Markup {
    let style = format!("--d:{}", depth.min(STACK_MAX_DEPTH));
    markup! {
        div class="stack-row" style=(style) {
            @if let Some(marker) = frame.marker {
                (stack_marker_chip(marker))
            }
            span class="sym" { (frame.symbol.as_str()) }
            @if let Some(note) = &frame.note {
                span class="note" { (note.as_str()) }
            }
        }
        @for child in &frame.children {
            (stack_frame(child, depth + 1))
        }
    }
}

/// render.py's `STACK_MARKS` (`render.py:195-196`) plus the chip it builds
/// (`render.py:220`): a `.mark mark-{cls}` span, its visible text the raw
/// mark glyph itself (`+`/`-`/`~`/`->`), its `title` the English label.
/// `Called` is the typed replacement for the old text grammar's `->` token
/// (`("keep", "untouched")`) — see [`StackMarker`]'s own doc comment.
fn stack_marker_chip(marker: StackMarker) -> Markup {
    let (class, symbol, title) = match marker {
        StackMarker::Added => ("mark-add", "+", "added"),
        StackMarker::Removed => ("mark-del", "-", "removed"),
        StackMarker::Changed => ("mark-chg", "~", "changed"),
        StackMarker::Called => ("mark-keep", "->", "untouched"),
    };
    markup! {
        span class=(format!("mark {class}")) title=(title) { (symbol) }
    }
}

// --- 16. `Examples` --------------------------------------------------------------

/// A worked-example figure: a three-column `given / when / then` table, one row
/// per [`WorkedExample`], each row coloured by its [`ExampleKind`]. Cell text
/// renders as inline Markdown (so `` `LPUSH k a b c` `` reads as code), the same
/// free-text treatment the impact and decision cells use.
pub(super) fn examples_markup(
    caption: Option<&NonEmptyText>,
    rows: &OneOrMore<WorkedExample>,
) -> Markup {
    markup! {
        div class="d examples" {
            table class="ex" {
                thead { tr { th { "Given" } th { "When" } th { "Then" } } }
                tbody {
                    @for row in rows.as_slice() {
                        tr class=(example_row_class(row.kind)) {
                            td class="ex-g" {
                                @if let Some(given) = &row.given { (inline_markup(given.as_str())) } @else { "—" }
                            }
                            td class="ex-w" { (inline_markup(row.when.as_str())) }
                            td class="ex-t" { (inline_markup(row.then.as_str())) }
                        }
                    }
                }
            }
            @if let Some(caption) = caption {
                div class="d-cap" { (caption.as_str()) }
            }
        }
    }
}

/// The row class for a worked example's kind. An unmarked row defaults to the
/// neutral `ex-wire` token (which carries no tint) rather than a bare
/// class-less `<tr>`, matching `matrix_cell`/`compare_item`'s "never emit a
/// class-less row" convention; `boundary` draws warning attention, `error`
/// reuses the error token.
const fn example_row_class(kind: Option<ExampleKind>) -> &'static str {
    match kind {
        Some(ExampleKind::Wire) | None => "ex-wire",
        Some(ExampleKind::Boundary) => "ex-boundary",
        Some(ExampleKind::Error) => "ex-error",
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
    use super::super::diagram_markup;
    use crate::model::diagram::Diagram;

    fn render(json: serde_json::Value) -> String {
        let diagram: Diagram = serde_json::from_value(json).expect("valid diagram JSON");
        diagram_markup(&diagram).into_string()
    }

    // --- Er: unrepresentable pk+fk + badge rendering -----------------------------

    #[test]
    fn test_er_field_key_cannot_carry_both_primary_and_foreign_facts() {
        let json = serde_json::json!({
            "kind": "er",
            "entities": [{
                "name": "Order",
                "kind": null,
                "fields": [{
                    "name": "id",
                    "type": "uuid",
                    "key": {"kind": "primary", "references": "orders.id"}
                }]
            }],
            "relations": []
        });

        serde_json::from_value::<Diagram>(json)
            .expect_err("a field key cannot carry both a primary and a foreign fact");
    }

    #[test]
    fn test_er_primary_field_renders_the_pk_badge() {
        let html = render(serde_json::json!({
            "kind": "er",
            "entities": [{
                "name": "Order",
                "kind": null,
                "fields": [{"name": "id", "type": "uuid", "key": {"kind": "primary"}}]
            }],
            "relations": []
        }));

        assert!(html.contains("class=\"bdg pk\""));
    }

    #[test]
    fn test_er_foreign_field_renders_the_fk_badge_with_its_target() {
        let html = render(serde_json::json!({
            "kind": "er",
            "entities": [{
                "name": "Order",
                "kind": null,
                "fields": [{
                    "name": "customer_id",
                    "type": "uuid",
                    "key": {"kind": "foreign", "references": "customers.id"}
                }]
            }],
            "relations": []
        }));

        assert!(html.contains("class=\"bdg fk\""));
        assert!(html.contains("customers.id"));
    }

    #[test]
    fn test_er_new_entity_appends_the_kind_as_a_bare_class_modifier() {
        let html = render(serde_json::json!({
            "kind": "er",
            "entities": [{
                "name": "Order",
                "kind": "new",
                "fields": [{"name": "id", "type": "uuid", "key": null}]
            }],
            "relations": []
        }));

        assert!(html.contains("class=\"er-e new\""));
    }

    // --- Endpoint: `responses` `OneOrMore`, warning, auth icons, JSON body ------

    fn endpoint_json(responses: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "kind": "endpoint",
            "method": "get",
            "path": "/orders",
            "summary": null,
            "auth": {"kind": "open", "reason": "public read"},
            "params": [],
            "request": null,
            "responses": responses
        })
    }

    #[test]
    fn test_endpoint_rejects_zero_responses() {
        let json = endpoint_json(&serde_json::json!([]));

        serde_json::from_value::<Diagram>(json).expect_err("zero responses must be rejected");
    }

    #[test]
    fn test_endpoint_fewer_than_two_responses_renders_the_incompleteness_warning() {
        let html = render(endpoint_json(&serde_json::json!([{
            "status": 200, "description": "ok", "kind": null, "headers": [],
            "body": {"content_type": null, "example": null}
        }])));

        assert!(html.contains("class=\"ep-warn\""));
    }

    #[test]
    fn test_endpoint_two_responses_without_a_failure_status_renders_the_incompleteness_warning() {
        let html = render(endpoint_json(&serde_json::json!([
            {"status": 200, "description": "ok", "kind": null, "headers": [], "body": {"content_type": null, "example": null}},
            {"status": 201, "description": "created", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
        ])));

        assert!(html.contains("class=\"ep-warn\""));
    }

    #[test]
    fn test_endpoint_two_responses_with_a_failure_status_omits_the_incompleteness_warning() {
        let html = render(endpoint_json(&serde_json::json!([
            {"status": 200, "description": "ok", "kind": null, "headers": [], "body": {"content_type": null, "example": null}},
            {"status": 404, "description": "not found", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
        ])));

        assert!(!html.contains("ep-warn"));
    }

    #[test]
    fn test_endpoint_open_auth_renders_the_open_auth_class() {
        let mut json = endpoint_json(&serde_json::json!([
            {"status": 200, "description": "ok", "kind": null, "headers": [], "body": {"content_type": null, "example": null}},
            {"status": 404, "description": "nf", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
        ]));
        json["auth"] = serde_json::json!({"kind": "open", "reason": "public read"});

        let html = render(json);

        assert!(html.contains("class=\"ep-auth open\""));
    }

    #[test]
    fn test_endpoint_scheme_auth_renders_the_bare_ep_auth_class() {
        let mut json = endpoint_json(&serde_json::json!([
            {"status": 200, "description": "ok", "kind": null, "headers": [], "body": {"content_type": null, "example": null}},
            {"status": 404, "description": "nf", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
        ]));
        json["auth"] = serde_json::json!({"kind": "scheme", "name": "bearer"});

        let html = render(json);

        assert!(html.contains("class=\"ep-auth\""));
        assert!(!html.contains("ep-auth open"));
    }

    #[test]
    fn test_endpoint_response_row_carries_its_kind_as_the_tr_class() {
        let html = render(endpoint_json(&serde_json::json!([
            {"status": 201, "description": "created", "kind": "new", "headers": [], "body": {"content_type": null, "example": null}},
            {"status": 404, "description": "nf", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
        ])));

        assert!(html.contains("<tr class=\"new\">"));
    }

    #[test]
    fn test_endpoint_json_body_example_renders_highlighted() {
        let json = endpoint_json(&serde_json::json!([{
            "status": 200,
            "description": "ok",
            "kind": null,
            "headers": [],
            "body": {"content_type": "application/json", "example": "{\"id\": 1}"}
        }]));

        let html = render(json);

        assert!(html.contains("class=\"ep-ex\""));
        // Syntax-highlighted output wraps tokens in styled spans rather than
        // emitting the raw source verbatim.
        assert!(html.contains("span style"));
    }

    // --- Matrix: risk cell class + unlisted cell renders `mx-same` ---------------

    #[test]
    fn test_matrix_risk_cell_renders_the_risk_colour_class() {
        let html = render(serde_json::json!({
            "kind": "matrix",
            "columns": ["dev", "prod"],
            "rows": [{
                "name": "timeout",
                "cells": [{"column": "prod", "value": "30s", "kind": "risk"}]
            }]
        }));

        assert!(html.contains("class=\"mx-risk\""));
    }

    #[test]
    fn test_matrix_unlisted_cell_for_a_declared_column_renders_mx_same_with_an_empty_code() {
        let html = render(serde_json::json!({
            "kind": "matrix",
            "columns": ["dev", "prod"],
            "rows": [{
                "name": "timeout",
                "cells": [{"column": "prod", "value": "30s", "kind": null}]
            }]
        }));

        // render.py never renders a bare `<td></td>` — every declared column
        // gets a cell, `kind` defaulting to the literal `same` token.
        assert!(
            html.contains("<td class=\"mx-same\"><code></code></td>"),
            "the `dev` column has no cell and must render `mx-same` with an empty code: {html}"
        );
    }

    // --- Compare: zero-item column rejected + add/del classes --------------------

    #[test]
    fn test_compare_rejects_a_column_with_zero_items() {
        let json = serde_json::json!({
            "kind": "compare",
            "columns": [{"label": "before", "items": []}]
        });

        serde_json::from_value::<Diagram>(json).expect_err("zero items must be rejected");
    }

    #[test]
    fn test_compare_item_kinds_render_add_and_del_classes() {
        let html = render(serde_json::json!({
            "kind": "compare",
            "columns": [{
                "label": "before",
                "items": [
                    {"text": "inline validation", "kind": "add"},
                    {"text": "shared helper", "kind": "del"}
                ]
            }]
        }));

        assert!(html.contains("class=\"cmp-add\""));
        assert!(html.contains("class=\"cmp-del\""));
    }

    #[test]
    fn test_compare_item_without_a_kind_renders_the_normal_class() {
        let html = render(serde_json::json!({
            "kind": "compare",
            "columns": [{"label": "before", "items": [{"text": "inline", "kind": null}]}]
        }));

        assert!(html.contains("class=\"cmp-normal\""));
    }

    #[test]
    fn test_compare_second_column_carries_the_cmp_c1_index_class() {
        let html = render(serde_json::json!({
            "kind": "compare",
            "columns": [
                {"label": "before", "items": [{"text": "a", "kind": null}]},
                {"label": "after", "items": [{"text": "b", "kind": null}]}
            ]
        }));

        assert!(html.contains("class=\"cmp-col cmp-c0\""));
        assert!(html.contains("class=\"cmp-col cmp-c1\""));
    }

    // --- Stack: zero frames rejected + depth starts at 0 for the root ------------

    #[test]
    fn test_stack_rejects_zero_frames() {
        let json = serde_json::json!({"kind": "stack", "frames": []});

        serde_json::from_value::<Diagram>(json).expect_err("empty frames must be rejected");
    }

    #[test]
    fn test_stack_root_frame_renders_at_depth_0_and_its_children_at_depth_1() {
        let html = render(serde_json::json!({
            "kind": "stack",
            "frames": [{
                "marker": null,
                "symbol": "main",
                "note": null,
                "children": [
                    {"marker": "called", "symbol": "parse", "note": null, "children": []},
                    {"marker": "called", "symbol": "run", "note": null, "children": []}
                ]
            }]
        }));

        assert!(html.contains("--d:0"));
        assert_eq!(html.matches("--d:1").count(), 2);
    }

    #[test]
    fn test_stack_called_marker_renders_the_mark_keep_class_with_the_arrow_glyph() {
        let html = render(serde_json::json!({
            "kind": "stack",
            "frames": [{
                "marker": null,
                "symbol": "main",
                "note": null,
                "children": [
                    {"marker": "called", "symbol": "parse", "note": null, "children": []}
                ]
            }]
        }));

        assert!(html.contains("class=\"mark mark-keep\" title=\"untouched\">-&gt;<"));
    }

    // --- Determinism + no-SVG across the 6 HTML drawers ---------------------------

    fn minimal_json_for_each_html_kind() -> Vec<(&'static str, serde_json::Value)> {
        vec![
            (
                "er",
                serde_json::json!({
                    "kind": "er",
                    "entities": [{
                        "name": "Order",
                        "kind": null,
                        "fields": [{"name": "id", "type": "uuid", "key": {"kind": "primary"}}]
                    }],
                    "relations": [{"from": "Order", "to": "Customer", "kind": "many_to_one", "label": null}]
                }),
            ),
            (
                "endpoint",
                endpoint_json(&serde_json::json!([
                    {"status": 200, "description": "ok", "kind": null, "headers": [], "body": {"content_type": null, "example": null}},
                    {"status": 404, "description": "not found", "kind": null, "headers": [], "body": {"content_type": null, "example": null}}
                ])),
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
                    "rows": [{"given": null, "when": "LPUSH k a", "then": "a", "kind": null}]
                }),
            ),
        ]
    }

    #[test]
    fn test_each_html_drawer_renders_no_svg_and_is_deterministic() {
        for (kind, json) in minimal_json_for_each_html_kind() {
            let once = render(json.clone());
            let twice = render(json.clone());
            assert_eq!(once, twice, "`{kind}` must render deterministically");
            // `dg-svg` is `svg_root`'s own class, i.e. the diagram-canvas
            // wrapper the SVG family uses; a small inline `Icon` glyph
            // (`class="dg-icon"`, e.g. Er's pk/fk badges) is not that, and is
            // expected here.
            assert!(
                !once.contains("dg-svg"),
                "`{kind}` must not render an SVG diagram canvas: {once}"
            );
        }
    }
}
