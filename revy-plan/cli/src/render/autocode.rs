//! Render-side auto-monospacing of high-confidence code shapes in prose.
//!
//! The planner is asked to backtick the code it names (craft.md), but a bare
//! `run_render` or `main.rs:584` slips through. This pass wraps the tokens whose
//! *shape* is unambiguously code so they render as `<code>`/`` `code` `` without
//! the author having marked every one — a safety net over authored backticks,
//! never a reinterpretation of prose. It recognises only:
//!
//! - a filename with a code extension (`main.rs`, `plan.html`),
//! - a `file.rs:NNN` / `file.rs:NNN-NNN` source anchor,
//! - a `snake_case` identifier (`run_render`, `save_text_atomic`),
//! - a trailing-bang `macro!` (`println!`, `eprintln!`),
//! - a `a::b::c` qualified path,
//! - a `a/b/c.rs` slash path,
//! - a `revy plan <sub>` command invocation.
//!
//! A bare single-word CamelCase symbol (`RenderArgs`) is ambiguous and is left to
//! the author. Text already inside a code span or a URL is never wrapped: the
//! two render paths that consume [`segment_prose`] each skip those regions before
//! calling it (the HTML path processes only Markdown text nodes outside links;
//! [`autocode_markdown`] copies backtick spans and link destinations verbatim).

/// One run of rendered prose: passed through as-is, or wrapped as a code span.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Segment {
    Plain(String),
    Code(String),
}

/// File extensions treated as a code-shape signal on a filename or path — the
/// languages a plan cites plus the artifacts the renderer itself writes.
const CODE_EXTENSIONS: &[&str] = &[
    "rs", "py", "ts", "tsx", "js", "jsx", "go", "json", "yaml", "yml", "toml", "sh", "sql", "txt",
    "md", "html", "css",
];

/// Single-word macros a bare `name!` is treated as a macro for even without an
/// underscore — the print/format/assert family a Rust plan actually names. A
/// plain lowercase word plus `!` (`fast!`, `wow!`) is deliberately left alone, so
/// ordinary prose keeps its exclamation marks.
const KNOWN_MACROS: &[&str] = &[
    "vec",
    "format",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "todo",
    "unreachable",
    "unimplemented",
    "assert",
    "dbg",
    "matches",
    "concat",
    "env",
];

const fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

const fn is_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')
}

/// A token the scanner found at a position: wrap it as code, or copy it through
/// untouched (a URL, so an embedded `foo.rs` is never mistaken for a filename).
enum Found {
    Code(usize),
    Skip(usize),
}

/// Split `text` (a single plain-text run, no Markdown structure) into plain and
/// code segments. Concatenating the segment payloads reproduces `text` exactly.
pub(super) fn segment_prose(text: &str) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Segment> = Vec::new();
    let mut plain = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let prev = if i == 0 {
            None
        } else {
            chars.get(i - 1).copied()
        };
        match find_token(&chars, i, prev) {
            Some(Found::Code(len)) => {
                if !plain.is_empty() {
                    out.push(Segment::Plain(std::mem::take(&mut plain)));
                }
                out.push(Segment::Code(chars.iter().skip(i).take(len).collect()));
                i += len;
            }
            Some(Found::Skip(len)) => {
                for c in chars.iter().skip(i).take(len) {
                    plain.push(*c);
                }
                i += len;
            }
            None => {
                if let Some(c) = chars.get(i) {
                    plain.push(*c);
                }
                i += 1;
            }
        }
    }
    if !plain.is_empty() {
        out.push(Segment::Plain(plain));
    }
    out
}

/// Wrap code shapes in backticks in a Markdown string, leaving existing code
/// spans and link destinations verbatim so nothing is double-wrapped and no URL
/// is rewritten.
pub(super) fn autocode_markdown(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut plain = String::new();
    let mut i = 0usize;
    while let Some(c) = chars.get(i).copied() {
        if c == '`' {
            flush_plain(&mut out, &mut plain);
            out.push('`');
            i += 1;
            while let Some(inner) = chars.get(i).copied() {
                out.push(inner);
                i += 1;
                if inner == '`' {
                    break;
                }
            }
        } else if c == ']' && matches!(chars.get(i + 1), Some('(')) {
            // A Markdown link/image destination: copy `](url)` verbatim.
            flush_plain(&mut out, &mut plain);
            out.push(']');
            out.push('(');
            i += 2;
            while let Some(inner) = chars.get(i).copied() {
                out.push(inner);
                i += 1;
                if inner == ')' {
                    break;
                }
            }
        } else {
            plain.push(c);
            i += 1;
        }
    }
    flush_plain(&mut out, &mut plain);
    out
}

fn flush_plain(out: &mut String, plain: &mut String) {
    if plain.is_empty() {
        return;
    }
    for segment in segment_prose(plain) {
        match segment {
            Segment::Plain(text) => out.push_str(&text),
            Segment::Code(text) => {
                out.push('`');
                out.push_str(&text);
                out.push('`');
            }
        }
    }
    plain.clear();
}

fn find_token(chars: &[char], i: usize, prev: Option<char>) -> Option<Found> {
    // A URL is skipped whole, before the boundary gate, so a path inside it never
    // reaches the code matchers.
    if let Some(len) = match_url(chars, i) {
        return Some(Found::Skip(len));
    }
    // Every code shape must begin at an identifier/path boundary, so a shape is
    // never matched in the middle of a longer word.
    if prev.is_some_and(|c| is_ident_char(c) || matches!(c, '.' | '/' | ':')) {
        return None;
    }
    match_command(chars, i)
        .or_else(|| match_path(chars, i))
        .or_else(|| match_qualified(chars, i))
        .or_else(|| match_macro(chars, i))
        .or_else(|| match_snake(chars, i))
        .map(Found::Code)
}

fn starts_with(chars: &[char], i: usize, needle: &str) -> bool {
    let mut k = i;
    for want in needle.chars() {
        match chars.get(k) {
            Some(c) if *c == want => k += 1,
            _ => return false,
        }
    }
    true
}

fn read_ident(chars: &[char], i: usize) -> usize {
    let mut len = 0usize;
    while chars.get(i + len).copied().is_some_and(is_ident_char) {
        len += 1;
    }
    len
}

fn match_url(chars: &[char], i: usize) -> Option<usize> {
    let scheme = ["https://", "http://"]
        .into_iter()
        .find(|scheme| starts_with(chars, i, scheme))?;
    let mut len = scheme.chars().count();
    while chars.get(i + len).is_some_and(|c| !c.is_whitespace()) {
        len += 1;
    }
    Some(len)
}

fn match_command(chars: &[char], i: usize) -> Option<usize> {
    const PREFIX: &str = "revy plan ";
    if !starts_with(chars, i, PREFIX) {
        return None;
    }
    let start = PREFIX.chars().count();
    if !chars.get(i + start).is_some_and(char::is_ascii_lowercase) {
        return None;
    }
    let mut len = start;
    while chars
        .get(i + len)
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
    {
        len += 1;
    }
    Some(len)
}

fn match_path(chars: &[char], i: usize) -> Option<usize> {
    let mut run_len = 0usize;
    while chars.get(i + run_len).copied().is_some_and(is_path_char) {
        run_len += 1;
    }
    // Trailing `.`/`-`/`/` is sentence or inline punctuation, not part of a path.
    while run_len > 0
        && chars
            .get(i + run_len - 1)
            .is_some_and(|c| matches!(c, '.' | '-' | '/'))
    {
        run_len -= 1;
    }
    if run_len == 0 {
        return None;
    }
    let run: String = chars.iter().skip(i).take(run_len).collect();
    if !has_code_extension(&run) {
        return None;
    }
    let line = match_line_suffix(chars, i + run_len).unwrap_or(0);
    Some(run_len + line)
}

fn has_code_extension(run: &str) -> bool {
    match run.rsplit_once('.') {
        Some((name, ext)) => {
            !name.is_empty()
                && !ext.is_empty()
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && CODE_EXTENSIONS.contains(&ext)
        }
        None => false,
    }
}

fn match_line_suffix(chars: &[char], i: usize) -> Option<usize> {
    if !matches!(chars.get(i), Some(':')) {
        return None;
    }
    let mut len = 1usize;
    let first_digit = len;
    while chars.get(i + len).is_some_and(char::is_ascii_digit) {
        len += 1;
    }
    if len == first_digit {
        return None;
    }
    if matches!(chars.get(i + len), Some('-')) {
        let mut span = len + 1;
        let span_start = span;
        while chars.get(i + span).is_some_and(char::is_ascii_digit) {
            span += 1;
        }
        if span > span_start {
            len = span;
        }
    }
    Some(len)
}

fn match_qualified(chars: &[char], i: usize) -> Option<usize> {
    let mut len = read_ident(chars, i);
    if len == 0 {
        return None;
    }
    let mut saw_separator = false;
    while matches!(chars.get(i + len), Some(':')) && matches!(chars.get(i + len + 1), Some(':')) {
        let segment = read_ident(chars, i + len + 2);
        if segment == 0 {
            break;
        }
        len += 2 + segment;
        saw_separator = true;
    }
    saw_separator.then_some(len)
}

fn match_macro(chars: &[char], i: usize) -> Option<usize> {
    let first = chars.get(i)?;
    if !(first.is_ascii_lowercase() || *first == '_') {
        return None;
    }
    let len = read_ident(chars, i);
    if len < 2 {
        return None;
    }
    if !matches!(chars.get(i + len), Some('!')) {
        return None;
    }
    if chars.iter().skip(i).take(len).any(char::is_ascii_uppercase) {
        return None;
    }
    // Only a name that carries an underscore or is a known macro, so a plain
    // lowercase word plus `!` ("fast!") stays an exclamation, not a macro bang.
    let base: String = chars.iter().skip(i).take(len).collect();
    if !(base.contains('_') || KNOWN_MACROS.contains(&base.as_str())) {
        return None;
    }
    Some(len + 1)
}

fn match_snake(chars: &[char], i: usize) -> Option<usize> {
    let len = read_ident(chars, i);
    if len == 0 {
        return None;
    }
    let first = chars.get(i)?;
    let last = chars.get(i + len - 1)?;
    if *first == '_' || *last == '_' {
        return None;
    }
    let mut has_underscore = false;
    let mut has_lower = false;
    for c in chars.iter().skip(i).take(len) {
        has_underscore |= *c == '_';
        has_lower |= c.is_ascii_lowercase();
    }
    (has_underscore && has_lower).then_some(len)
}

#[cfg(test)]
mod tests {
    use super::{Segment, autocode_markdown, segment_prose};

    fn codes(text: &str) -> Vec<String> {
        segment_prose(text)
            .into_iter()
            .filter_map(|segment| match segment {
                Segment::Code(code) => Some(code),
                Segment::Plain(_) => None,
            })
            .collect()
    }

    fn roundtrip(text: &str) -> String {
        segment_prose(text)
            .into_iter()
            .map(|segment| match segment {
                Segment::Plain(text) | Segment::Code(text) => text,
            })
            .collect()
    }

    #[test]
    fn wraps_a_filename_with_a_code_extension() {
        assert_eq!(
            codes("it writes plan.md and plan.html"),
            ["plan.md", "plan.html"]
        );
    }

    #[test]
    fn wraps_a_file_line_anchor_and_a_range() {
        assert_eq!(
            codes("see main.rs:584 and main.rs:584-585 now"),
            ["main.rs:584", "main.rs:584-585"]
        );
    }

    #[test]
    fn wraps_a_slash_path() {
        assert_eq!(codes("edit src/queue.rs today"), ["src/queue.rs"]);
    }

    #[test]
    fn wraps_snake_case_identifiers() {
        assert_eq!(
            codes("run_render calls save_text_atomic once"),
            ["run_render", "save_text_atomic"]
        );
    }

    #[test]
    fn wraps_trailing_bang_macros_mid_sentence() {
        assert_eq!(
            codes("the two println! and eprintln! lines"),
            ["println!", "eprintln!"]
        );
    }

    #[test]
    fn wraps_a_qualified_path() {
        assert_eq!(
            codes("call RenderArgs::parse then a::b::c"),
            ["RenderArgs::parse", "a::b::c"]
        );
    }

    #[test]
    fn wraps_a_revy_plan_command() {
        assert_eq!(
            codes("run revy plan render --quiet next"),
            ["revy plan render"]
        );
    }

    #[test]
    fn leaves_bare_camelcase_to_the_author() {
        assert_eq!(
            codes("the RenderArgs struct grows a field"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn does_not_wrap_a_sentence_final_exclamation() {
        assert_eq!(
            codes("the queue is fast! it never blocks"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn never_wraps_a_token_inside_a_url() {
        assert_eq!(
            codes("see https://example.com/main.rs for details"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn preserves_the_original_text_across_segments() {
        let text = "run_render writes plan.md via println! per main.rs:12";
        assert_eq!(roundtrip(text), text);
    }

    #[test]
    fn markdown_does_not_double_wrap_an_existing_code_span() {
        assert_eq!(
            autocode_markdown("call `run_render` and try_render now"),
            "call `run_render` and `try_render` now"
        );
    }

    #[test]
    fn markdown_leaves_a_link_destination_untouched() {
        assert_eq!(
            autocode_markdown("[the source](src/main.rs) is here"),
            "[the source](src/main.rs) is here"
        );
    }
}
