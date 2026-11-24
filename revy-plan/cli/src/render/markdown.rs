//! Authored plan prose rendered as Markdown.
//!
//! Plan prose — narrative blocks, the scope/behavior sentences, glossary
//! definitions — is authored Markdown: `**bold**`, inline `` `code` ``, bullet
//! and numbered lists, GFM pipe tables, and `~~strikethrough~~`. Both entry
//! points render it through one shared comrak configuration so block and inline
//! contexts can never disagree on what a construct means.
//!
//! Raw HTML in prose is **escaped**, never passed through
//! (`render.escape = true`): authored plan prose is not a trusted HTML source,
//! so `<script>` renders as the literal text `&lt;script&gt;`. This is a
//! deliberate deviation from the render.py oracle, which enabled raw-HTML
//! passthrough — dropped here for security.

use std::sync::LazyLock;

use comrak::nodes::{AstNode, NodeCode, NodeValue};
use comrak::{Arena, Options, format_html, parse_document};
use maud::{Markup, PreEscaped};

use super::autocode::{Segment, segment_prose};

/// The one shared comrak configuration: `CommonMark` plus the two GFM extensions
/// plan prose actually uses (pipe tables and strikethrough), with raw HTML
/// escaped rather than passed through. `Options<'static>` is `Sync` — comrak's
/// only borrowing fields are callbacks bounded `Send + Sync` — so a single
/// process-wide instance is sound, matching this crate's `SYNTAX_SET`/`THEME`
/// idiom.
static OPTIONS: LazyLock<Options<'static>> = LazyLock::new(|| {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.render.escape = true;
    options
});

/// Render authored prose as block-level Markdown HTML: paragraphs, lists,
/// tables, and fenced code. For a block container (`Block::Prose`'s `<div>`)
/// only — never an inline context, since the output can be several sibling
/// block elements. Tables render as comrak's plain `<table>`, which `plan.css`'s
/// bare-`table` rules already style.
pub(super) fn block_html(text: &str) -> Markup {
    let arena = Arena::new();
    let root = parse_document(&arena, text, &OPTIONS);
    autocode(&arena, root);
    let mut html = String::new();
    let _ = format_html(root, &OPTIONS, &mut html);
    PreEscaped(html)
}

/// Render authored prose as inline HTML only: `<strong>`, `<em>`, `<code>`,
/// `<del>`, links, with **no** block-level tags whatever the input. For the
/// inline sites — a `<p>`/`<dt>`/`<dd>` body — where a `<ul>`/`<table>`/
/// `<blockquote>`/`<h1>` cannot legally nest (the browser would force-close the
/// enclosing `<p>` and hoist the block out, corrupting the DOM).
///
/// Every block wrapper the author writes (a list, pipe table, blockquote,
/// heading) is flattened away, keeping only its inline content; sibling block
/// groups (paragraphs, list items, cells) join with a single space so text
/// never runs together. Raw HTML — inline or a whole block — is escaped, and a
/// code/HTML block's literal is kept as escaped text, so no authored content is
/// lost and no live tag can be emitted.
pub(super) fn inline_markup(text: &str) -> Markup {
    let arena = Arena::new();
    let root = parse_document(&arena, text, &OPTIONS);
    autocode(&arena, root);
    let mut html = String::new();
    push_inline(root, &mut html);
    PreEscaped(html)
}

/// Wrap high-confidence code shapes in the parsed prose in `<code>` spans, in
/// place, before it is formatted (see [`super::autocode`]). Only Markdown text
/// nodes are rewritten, and never one inside a link label or an escaped span, so
/// an existing code span or a URL is left exactly as authored.
fn autocode<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>) {
    // Collect the target text nodes first: the walk borrows the tree, the rewrite
    // below mutates its sibling links.
    let mut targets: Vec<(&'a AstNode<'a>, String)> = Vec::new();
    for node in root.descendants() {
        let literal = match &node.data.borrow().value {
            NodeValue::Text(text) => Some(text.to_string()),
            _ => None,
        };
        if let Some(literal) = literal
            && !skip_autocode(node)
        {
            targets.push((node, literal));
        }
    }
    for (node, literal) in targets {
        let segments = segment_prose(&literal);
        if segments
            .iter()
            .all(|segment| matches!(segment, Segment::Plain(_)))
        {
            continue;
        }
        for segment in segments {
            let value = match segment {
                Segment::Plain(text) => NodeValue::Text(text.into()),
                Segment::Code(text) => NodeValue::Code(NodeCode {
                    num_backticks: 1,
                    literal: text,
                }),
            };
            node.insert_before(arena.alloc(AstNode::from(value)));
        }
        node.detach();
    }
}

/// A text node the autocode pass must not touch: a link/image label (whose
/// destination and text are the author's own), or an escaped span (which admits
/// only text children).
fn skip_autocode<'a>(node: &'a AstNode<'a>) -> bool {
    if node
        .parent()
        .is_some_and(|parent| matches!(parent.data.borrow().value, NodeValue::Escaped))
    {
        return true;
    }
    node.ancestors().skip(1).any(|ancestor| {
        matches!(
            ancestor.data.borrow().value,
            NodeValue::Link(_) | NodeValue::Image(_)
        )
    })
}

/// Append `node`'s inline rendering to `out`, dropping every block wrapper.
/// An inline node renders its whole inline subtree via comrak's own formatter;
/// a block node contributes only its inline descendants, its own tag discarded.
fn push_inline<'a>(node: &'a AstNode<'a>, out: &mut String) {
    let is_block;
    let literal;
    {
        let ast = node.data.borrow();
        is_block = ast.value.block();
        // Code and raw-HTML blocks hold their text in `literal`, not in child
        // nodes; recursing would silently drop it, so capture it here.
        literal = match &ast.value {
            NodeValue::CodeBlock(block) => Some(block.literal.clone()),
            NodeValue::HtmlBlock(block) => Some(block.literal.clone()),
            _ => None,
        };
    }

    if !is_block {
        let _ = format_html(node, &OPTIONS, out);
        return;
    }

    separate(out);
    if let Some(text) = literal {
        push_escaped(&text, out);
        return;
    }
    for child in node.children() {
        push_inline(child, out);
    }
}

/// Ensure a block group is separated from prior output by whitespace, so
/// flattened paragraphs/items/cells never run their text together.
fn separate(out: &mut String) {
    if !out.is_empty() && !out.ends_with(|character: char| character.is_whitespace()) {
        out.push(' ');
    }
}

/// HTML-escape text into `out` — for a code/HTML block's literal, which is
/// rendered as plain inline text rather than passed through as markup.
fn push_escaped(text: &str, out: &mut String) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{block_html, inline_markup};

    #[test]
    fn block_html_renders_bold_as_a_strong_element() {
        let html = block_html("This is **important** work").into_string();
        assert!(
            html.contains("<strong>important</strong>"),
            "bold prose must render as <strong>: {html}"
        );
    }

    #[test]
    fn block_html_renders_inline_code_as_a_code_element() {
        let html = block_html("call `RedisCommand::parse` first").into_string();
        assert!(
            html.contains("<code>RedisCommand::parse</code>"),
            "inline code must render as <code>: {html}"
        );
    }

    #[test]
    fn block_html_renders_a_pipe_table_as_a_table_element() {
        let source = "| Command | Arity |\n|---|---|\n| GET | 2 |\n";
        let html = block_html(source).into_string();
        assert!(
            html.contains("<table>"),
            "a GFM pipe table must render as a <table>: {html}"
        );
        assert!(
            html.contains("<td>GET</td>"),
            "the table must carry its authored cells: {html}"
        );
    }

    #[test]
    fn block_html_renders_a_bullet_list_as_a_ul_of_list_items() {
        let html = block_html("- first\n- second\n").into_string();
        assert!(
            html.contains("<ul>") && html.contains("<li>first</li>"),
            "a bullet list must render as <ul><li>: {html}"
        );
    }

    #[test]
    fn block_html_renders_strikethrough_as_a_del_element() {
        let html = block_html("this is ~~gone~~ now").into_string();
        assert!(
            html.contains("<del>gone</del>"),
            "GFM strikethrough must render as <del>: {html}"
        );
    }

    #[test]
    fn block_html_escapes_raw_html_in_prose_rather_than_emitting_a_live_tag() {
        let html = block_html("danger <script>alert(1)</script> here").into_string();
        assert!(
            !html.contains("<script>"),
            "raw HTML in prose must never emit a live tag: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "raw HTML in prose must be escaped to text: {html}"
        );
    }

    #[test]
    fn block_html_escapes_a_raw_html_block_not_only_inline_html() {
        // A `<script>` alone on its line parses as an HtmlBlock, a distinct node
        // from the inline HtmlInline the test above covers.
        let html = block_html("intro paragraph\n\n<script>alert(1)</script>\n\nmore").into_string();
        assert!(
            !html.contains("<script>"),
            "a raw HTML block must never emit a live tag: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "a raw HTML block must be escaped to text: {html}"
        );
    }

    #[test]
    fn inline_markup_renders_a_code_span_without_wrapping_it_in_a_paragraph() {
        let html = inline_markup("edit `src/server.rs` now").into_string();
        assert_eq!(html, "edit <code>src/server.rs</code> now");
    }

    #[test]
    fn inline_markup_renders_bold_inline_with_no_block_wrapper() {
        let html = inline_markup("a **bold** run").into_string();
        assert_eq!(html, "a <strong>bold</strong> run");
    }

    #[test]
    fn inline_markup_escapes_raw_html_rather_than_emitting_a_live_tag() {
        let html = inline_markup("see <img src=x onerror=alert(1)>").into_string();
        assert!(
            !html.contains("<img"),
            "inline raw HTML must not emit a live tag: {html}"
        );
        assert!(
            html.contains("&lt;img"),
            "inline raw HTML must be escaped to text: {html}"
        );
    }

    #[test]
    fn inline_markup_flattens_a_bullet_list_to_inline_text_with_no_block_tags() {
        let html = inline_markup("- **first** item\n- second item").into_string();
        assert!(
            !html.contains("<ul>") && !html.contains("<li>"),
            "a list in an inline field must emit no block tags: {html}"
        );
        assert!(
            html.contains("<strong>first</strong>"),
            "inline formatting inside a flattened list must survive: {html}"
        );
        assert!(
            html.contains("first") && html.contains("second"),
            "flattened list item text must not be dropped: {html}"
        );
    }

    #[test]
    fn inline_markup_flattens_a_pipe_table_to_inline_text_with_no_table_tags() {
        let html = inline_markup("| A | B |\n|---|---|\n| xcell | ycell |").into_string();
        assert!(
            !html.contains("<table>") && !html.contains("<td>") && !html.contains("<tr>"),
            "a pipe table in an inline field must emit no block tags: {html}"
        );
        assert!(
            html.contains("xcell") && html.contains("ycell"),
            "flattened table cell text must not be dropped: {html}"
        );
    }

    #[test]
    fn inline_markup_flattens_a_blockquote_to_inline_text_with_no_blockquote_tag() {
        let html = inline_markup("> quoted **words**").into_string();
        assert!(
            !html.contains("<blockquote>"),
            "a blockquote in an inline field must emit no block tag: {html}"
        );
        assert!(
            html.contains("<strong>words</strong>"),
            "inline formatting inside a flattened blockquote must survive: {html}"
        );
    }

    #[test]
    fn inline_markup_flattens_a_heading_to_inline_text_with_no_heading_tag() {
        let html = inline_markup("# A heading").into_string();
        assert!(
            !html.contains("<h1>") && !html.contains("<h2>"),
            "a heading in an inline field must emit no block tag: {html}"
        );
        assert!(
            html.contains("A heading"),
            "flattened heading text must not be dropped: {html}"
        );
    }

    #[test]
    fn inline_markup_escapes_a_raw_html_block_flattened_into_an_inline_field() {
        let html = inline_markup("<script>alert(1)</script>").into_string();
        assert!(
            !html.contains("<script>"),
            "a raw HTML block flattened inline must never emit a live tag: {html}"
        );
        assert!(
            html.contains("&lt;script&gt;"),
            "a raw HTML block flattened inline must be escaped: {html}"
        );
    }

    #[test]
    fn inline_markup_joins_sibling_paragraphs_with_a_single_space() {
        let html = inline_markup("First paragraph.\n\nSecond paragraph.").into_string();
        assert_eq!(html, "First paragraph. Second paragraph.");
    }

    #[test]
    fn inline_markup_auto_monospaces_a_bare_code_shape() {
        let html = inline_markup("run_render writes to main.rs:584").into_string();
        assert_eq!(
            html,
            "<code>run_render</code> writes to <code>main.rs:584</code>"
        );
    }

    #[test]
    fn block_html_auto_monospaces_a_bare_snake_case_identifier() {
        let html = block_html("It calls save_text_atomic first").into_string();
        assert!(
            html.contains("<code>save_text_atomic</code>"),
            "a bare snake_case identifier must render as <code>: {html}"
        );
    }

    #[test]
    fn autocode_does_not_double_wrap_an_authored_code_span() {
        let html = inline_markup("call `run_render` now").into_string();
        assert_eq!(html, "call <code>run_render</code> now");
    }

    #[test]
    fn autocode_leaves_a_bare_url_untouched() {
        let html = inline_markup("see https://example.com/main.rs today").into_string();
        assert!(
            !html.contains("<code>"),
            "a token inside a URL must never be wrapped: {html}"
        );
    }

    #[test]
    fn autocode_leaves_bare_camelcase_to_the_author() {
        let html = inline_markup("the RenderArgs struct grows").into_string();
        assert!(
            !html.contains("<code>"),
            "a bare CamelCase symbol is ambiguous and stays plain: {html}"
        );
    }

    #[test]
    fn autocode_monospaces_tokens_after_an_authored_code_span() {
        // A run that already carries one authored code span must still have its
        // later bare shapes wrapped — the pass covers the whole run.
        let html =
            inline_markup("call `run_serve` then serve::run and save_text_atomic").into_string();
        assert!(html.contains("<code>run_serve</code>"), "{html}");
        assert!(html.contains("<code>serve::run</code>"), "{html}");
        assert!(html.contains("<code>save_text_atomic</code>"), "{html}");
    }
}
