// Pure token transforms over a parsed plan: glossary lists become definition lists, long lists and tables fold
// their tail, and a folded section gets a one-line digest. `Token` is markdown-it's token class.
import { escapeHtml } from './escape.mjs';
import { COMPONENT_LANGS } from './components/index.mjs';
import { DIAGRAM_LANGS } from './diagram.mjs';
import { FIGURE_LANGS } from './figure.mjs';
import { plainText } from './markdown.mjs';
import { isHunked, parseDiff, rowOf, spanText } from './diff.mjs';
import { diffLocationOf, locationOf } from './paths.mjs';

const LIST_OPEN = new Set(['bullet_list_open', 'ordered_list_open']);
const html = (Token, content, meta) => Object.assign(new Token('html_block', '', 0), { content, meta });
const closeOf = (tokens, open) => tokens.findIndex((t, j) => j > open && t.level === tokens[open].level && t.type === tokens[open].type.replace('_open', '_close'));

/** A text token's content split at its first `: `, as the term and the definition of a glossary entry. */
function splitTerm(children, Token) {
  const at = children.findIndex((c) => c.type === 'text' && /:\s/.test(c.content));
  if (at === -1) return undefined;
  const c = children[at];
  const k = c.content.search(/:\s/);
  const text = (content) => Object.assign(new Token('text', '', 0), { content, meta: c.meta });
  const [head, rest] = [c.content.slice(0, k), c.content.slice(k + 1).trimStart()];
  const term = [...children.slice(0, at), ...(head ? [text(head)] : [])];
  const def = [...(rest ? [text(rest)] : []), ...children.slice(at + 1)];
  return term.length && def.length && !term.some((t) => t.type === 'softbreak') ? { term, def } : undefined;
}

/** A heading containing "Glossary", or a paragraph label containing it and ending in a colon. */
function labelledGlossary(tokens, open) {
  const [close, inline] = [tokens[open - 1], tokens[open - 2]];
  if (close?.type === 'heading_close') return /glossary/i.test(inline.content);
  return close?.type === 'paragraph_close' && /glossary/i.test(inline.content) && /:\s*$/.test(inline.content);
}

/** The entries of a flat list whose every item is one `term: definition` paragraph; undefined otherwise. */
function glossaryEntries(tokens, open, close, Token) {
  const body = tokens.slice(open + 1, close);
  if (body.length % 5 !== 0) return undefined;
  const items = Array.from({ length: body.length / 5 }, (_, i) => body.slice(i * 5, i * 5 + 5));
  const shaped = items.every(([a, b, c, d, e]) => a.type === 'list_item_open' && b.type === 'paragraph_open'
    && c.type === 'inline' && d.type === 'paragraph_close' && e.type === 'list_item_close');
  if (!shaped) return undefined;
  const entries = items.map(([, , inline]) => splitTerm(inline.children ?? [], Token));
  return entries.every(Boolean) ? entries : undefined;
}

export function glossaries(tokens, Token) {
  const out = [];
  for (let i = 0; i < tokens.length; i += 1) {
    const close = tokens[i].type === 'bullet_list_open' && labelledGlossary(tokens, i) ? closeOf(tokens, i) : -1;
    const entries = close === -1 ? undefined : glossaryEntries(tokens, i, close, Token);
    if (!entries) {
      out.push(tokens[i]);
      continue;
    }
    const inline = (children, glossary) => Object.assign(new Token('inline', '', 0), { children, content: '', meta: { glossary } });
    out.push(html(Token, '<dl class="glossary">\n'), ...entries.flatMap(({ term, def }) => [
      html(Token, '<div><dt>', { term: true }), inline(term, 'term'), html(Token, '</dt><dd>'), inline(def, 'def'), html(Token, '</dd></div>\n'),
    ]), html(Token, '</dl>\n'));
    i = close;
  }
  return out;
}

const disclosure = (Token, n, what) => html(Token,
  `<details class="tail"><summary><span class="tail-more">show ${n} more${what}</span><span class="tail-less">show fewer</span></summary></details>\n`);

/** Items of a list and body rows of a table past the first `tail` are marked, and a disclosure after it reveals them. */
export function foldTails(tokens, tail, Token) {
  if (tail === null) return tokens;
  const after = new Map();
  tokens.forEach((t, i) => {
    const list = LIST_OPEN.has(t.type);
    if (!list && t.type !== 'tbody_open') return;
    // A snippet's line notes read with its code; folding them would hide what the code is shown for.
    if ((t.attrGet('class') ?? '').includes('snippet-notes')) return;
    const close = closeOf(tokens, i);
    const units = tokens.slice(i + 1, close).filter((u) => u.level === t.level + 1 && u.type === (list ? 'list_item_open' : 'tr_open'));
    if (units.length <= tail) return;
    units.slice(tail).forEach((u) => u.attrJoin('class', list ? 'tail-item' : 'tail-row'));
    const end = list ? close : tokens.findIndex((u, j) => j > close && u.type === 'table_close');
    after.set(end, disclosure(Token, units.length - tail, list ? '' : ' rows'));
  });
  return tokens.flatMap((t, i) => (after.has(i) ? [t, after.get(i)] : [t]));
}

const FIGURES = new Set([...COMPONENT_LANGS, ...DIAGRAM_LANGS, ...FIGURE_LANGS]);
const plural = (n, word) => `${n} ${word}${n === 1 ? '' : 's'}`;
const MAX_SENTENCE = 120;

const bodyRows = (tokens) => tokens.reduce((acc, t) => {
  if (t.type === 'tbody_open' || t.type === 'tbody_close') return { ...acc, inBody: t.type === 'tbody_open' };
  return t.type === 'tr_open' && acc.inBody ? { ...acc, n: acc.n + 1 } : acc;
}, { inBody: false, n: 0 }).n;

/** One line saying what a folded section holds: its counts, or else its first sentence. Escaped HTML, or ''. */
export function digestOf(tokens) {
  const lang = (t) => t.info.trim().split(/\s+/)[0].toLowerCase();
  const fences = tokens.filter((t) => t.type === 'fence');
  const counts = [
    [bodyRows(tokens), 'row'],
    [tokens.filter((t) => t.type === 'list_item_open' && t.level === 1).length, 'item'],
    [tokens.filter((t) => t.meta?.term).length, 'term'],
    [fences.filter((t) => FIGURES.has(lang(t))).length, 'figure'],
    [fences.filter((t) => !FIGURES.has(lang(t))).length, 'code block'],
  ].filter(([n]) => n > 0);
  if (counts.length) return escapeHtml(counts.map(([n, word]) => plural(n, word)).join(' · '));
  const first = tokens.find((t, i) => t.type === 'inline' && tokens[i - 1]?.type === 'paragraph_open');
  if (!first) return '';
  const text = plainText(first).replace(/\s+/g, ' ').trim();
  const sentence = /^.*?[.!?](?=\s|$)/.exec(text)?.[0] ?? text;
  return escapeHtml(sentence.length > MAX_SENTENCE ? `${sentence.slice(0, MAX_SENTENCE - 1)}…` : sentence);
}

const NOTE = /^(-?L\d+(?:-L?\d+)?(?:\s*,\s*L\d+(?:-L?\d+)?)*)\s+[—–-]\s+/;

/** `L240-L241, L250` -> [[240, 241], [250, 250]] */
const rangesOf = (key) => key.split(/\s*,\s*/).map((r) => {
  const [a, b] = r.replace(/L/g, '').split('-').map(Number);
  return [a, b ?? a];
});

/** The notes of a fence: the list or paragraph right after it whose every item or line starts `L<n> — `. */
function notesAfter(tokens, i) {
  const next = tokens[i + 1];
  if (next?.type === 'paragraph_open') {
    const inline = tokens[i + 2];
    const lines = inline.content.split('\n');
    return lines.every((l) => NOTE.test(l)) ? { kind: 'paragraph', end: i + 3, inline } : undefined;
  }
  if (next?.type !== 'bullet_list_open') return undefined;
  const close = closeOf(tokens, i + 1);
  const body = tokens.slice(i + 2, close);
  if (body.length % 5 !== 0) return undefined;
  const items = Array.from({ length: body.length / 5 }, (_, k) => body.slice(k * 5, k * 5 + 5));
  const ok = items.every(([a, b, c]) => a.type === 'list_item_open' && b.type === 'paragraph_open' && c.type === 'inline' && NOTE.test(c.content));
  return ok ? { kind: 'list', end: close, items } : undefined;
}

/**
 * The chunks notes mark: a note whose lines make one span of rows (`span`, from its frame) covers those rows as a
 * chunk. A chunk overlapping an earlier one stays a plain note; a chunk touching the one before it takes the other tint.
 * @param {{ line: number, span: { first: number, last: number, label: string, name: string } | undefined }[]} notes
 * @param {string} kind `snippet` or `diff`, for the report
 * @returns {{ chunks: { note: number, first: number, last: number, name: string, alt: boolean }[], warnings: { text: string, line: number }[] }}
 */
export function chunksOf(notes, kind) {
  const warnings = [];
  const taken = [];
  notes.forEach(({ span, line }, note) => {
    if (!span) return;
    const clash = taken.find((c) => span.first <= c.last && c.first <= span.last);
    if (clash) {
      warnings.push({ text: `${kind} note ${span.label} overlaps the chunk ${clash.label}; only the first is a chunk`, line });
      return;
    }
    taken.push({ note, ...span });
  });
  const chunks = [...taken].sort((x, y) => x.first - y.first).reduce((done, { label, ...c }) => {
    const prev = done.at(-1);
    return [...done, { ...c, alt: Boolean(prev && prev.last + 1 === c.first && !prev.alt) }];
  }, []);
  return { chunks, warnings };
}

/**
 * @typedef {{ kind: 'snippet' | 'diff', outside: (side: 'head' | 'base', ranges: [number, number][], key: string) => string[],
 *   target: (side: 'head' | 'base', n: number) => string | undefined,
 *   rows: (side: 'head' | 'base', a: number, b: number) => [number, number] | undefined }} Frame
 *   How a block's notes find their lines: the reports for lines it lacks, the row id a line links to, and the rows a
 *   range covers (undefined when it is not one run of rows).
 */

/** The frame of a code snippet numbered `start`..`end`: rows are its lines. */
const codeFrame = (id, start, end) => ({
  kind: 'snippet',
  outside: (side, ranges, key) => (side === 'base'
    ? [`snippet note ${key} names base lines; only a located diff has them`]
    : ranges.flat().filter((n) => n < start || n > end).map((n) => `snippet note L${n} names a line outside the snippet (${start}-${end})`)),
  target: (side, n) => (side === 'head' && n >= start && n <= end ? `${id}-L${n}` : undefined),
  rows: (side, a, b) => (side === 'head' && a >= start && b <= end ? [a - start, b - start] : undefined),
});

/** The id of a diff row: by its head line, or its base line when it has none. */
export const diffRowId = (id, row) => (row.head === undefined ? `${id}-B${row.base}` : `${id}-L${row.head}`);

/** The frame of a located diff: rows are its hunk headers and code lines; a range must stay inside one hunk. */
const diffFrame = (id, diff) => ({
  kind: 'diff',
  outside: (side, ranges, key) => (ranges.some(([a, b]) => Array.from({ length: Math.max(b - a + 1, 1) }, (_, k) => a + k).some((n) => rowOf(diff, side, n) < 0))
    ? [`diff note ${key} names ${side} lines outside the hunks (${spanText(diff.spans[side])})`]
    : []),
  target: (side, n) => {
    const row = diff.rows[rowOf(diff, side, n)];
    return row && diffRowId(id, row);
  },
  rows: (side, a, b) => {
    const [first, last] = [rowOf(diff, side, a), rowOf(diff, side, b)];
    return first >= 0 && last >= 0 && !diff.rows.slice(first, last).some((r) => r.kind === 'hunk') ? [first, last] : undefined;
  },
});

/** What a fence's notes are measured against, recorded on the fence: a located diff, a snippet, or nothing. */
function frameOf(fence, notes, warnings) {
  const id = `snip-${fence.map[0] + 1}`;
  const lang = fence.info.trim().split(/\s+/)[0].toLowerCase();
  const at = lang === 'diff' ? diffLocationOf(fence.info) : undefined;
  if (at && isHunked(fence.content)) {
    const { diff, warnings: found } = parseDiff(fence.content);
    found.forEach((text) => warnings.push({ text, line: fence.map[0] + 1 }));
    if (diff) {
      fence.meta = { ...fence.meta, diff: { id, at, diff, lang: at.rel.split('.').pop().toLowerCase(), chunks: [] } };
      return { id, frame: diffFrame(id, diff), record: fence.meta.diff };
    }
    return undefined;
  }
  const where = locationOf(fence.info);
  if (!where && !notes) return undefined;
  const start = where?.start ?? 1;
  const end = where?.end ?? start + fence.content.replace(/\n$/, '').split('\n').length - 1;
  fence.meta = { ...fence.meta, snippet: { id, start, end, chunks: [] } };
  return { id, frame: codeFrame(id, start, end), record: fence.meta.snippet };
}

/**
 * Code fences with a location (```` ```rust src/a.rs:240-260 ````), located diffs (```` ```diff src/a.rs ```` with
 * `@@` hunks), or code with line notes after it become numbered blocks: the fence gets `meta.snippet` or `meta.diff`,
 * and each note (`L<n> — `, or `-L<n> — ` for a diff's base lines) is tied to its lines (`data-lines`, `data-side`)
 * and links the first of them. A note covering one run of rows is a chunk (see `chunksOf`) and its item gets the id
 * the chunk points at. A note naming a line the block lacks is reported.
 * @returns {{ tokens: object[], warnings: { text: string, line: number }[] }}
 */
export function snippetNotes(tokens, Token) {
  const warnings = [];
  const out = [];
  for (let i = 0; i < tokens.length; i += 1) {
    const fence = tokens[i];
    out.push(fence);
    if (fence.type !== 'fence' || FIGURES.has(fence.info.trim().split(/\s+/)[0].toLowerCase())) continue;
    const notes = notesAfter(tokens, i);
    const framed = frameOf(fence, notes, warnings);
    if (!framed || !notes) continue;
    const { id, frame, record } = framed;
    /** The note's key as a link and its lines checked; the key is cut from the note's first text. */
    const tie = (inline, line) => {
      const first = (inline.children ?? []).find((c) => c.type === 'text');
      const key = NOTE.exec(first.content)[1];
      first.content = first.content.replace(NOTE, '');
      const side = key.startsWith('-') ? 'base' : 'head';
      const sign = side === 'base' ? '-' : '';
      const ranges = rangesOf(key.replace(/^-/, ''));
      ranges.filter(([a, b]) => a > b).forEach(([a, b]) => warnings.push({ text: `${frame.kind} note ${sign}L${a}-L${b} runs backwards; write ${sign}L${b}-L${a}`, line }));
      frame.outside(side, ranges, key).forEach((text) => warnings.push({ text, line }));
      const href = frame.target(side, ranges[0][0]);
      inline.children.unshift(html(Token, href ? `<a class="ln" href="#${href}">${key}</a> ` : `<span class="ln">${key}</span> `));
      const [[a, b]] = ranges;
      const rows = ranges.length === 1 && a < b ? frame.rows(side, a, b) : undefined;
      const span = rows && { first: rows[0], last: rows[1], label: `${sign}L${a}-L${b}`, name: `${side === 'base' ? 'Base lines' : 'Lines'} ${a}–${b}` };
      const attrs = [['data-lines', ranges.map(([x, y]) => (x === y ? `${x}` : `${x}-${y}`)).join(',')], ...(side === 'base' ? [['data-side', 'base']] : [])];
      return { line, span, attrs };
    };
    const noteId = (k) => `${id}-n${k + 1}`;
    /** Records the chunks the tied notes mark on the fence; returns the indexes of the notes that are chunks. */
    const chunk = (tied) => {
      const { chunks, warnings: overlaps } = chunksOf(tied, frame.kind);
      warnings.push(...overlaps);
      record.chunks = chunks.map((c) => ({ ...c, note: noteId(c.note) }));
      return new Set(chunks.map((c) => c.note));
    };
    if (notes.kind === 'list') {
      tokens[i + 1].attrJoin('class', 'snippet-notes');
      tokens[i + 1].attrSet('data-snippet', id);
      const tied = notes.items.map(([, , inline]) => tie(inline, inline.map[0] + 1));
      const chunked = chunk(tied);
      notes.items.forEach(([item], k) => {
        if (chunked.has(k)) item.attrSet('id', noteId(k));
        tied[k].attrs.forEach(([name, value]) => item.attrSet(name, value));
      });
      continue;
    }
    // A paragraph of note lines becomes the same list, one item per line.
    const lines = [[]];
    (notes.inline.children ?? []).forEach((c) => (c.type === 'softbreak' ? lines.push([]) : lines.at(-1).push(c)));
    const first = notes.inline.map[0] + 1;
    const inlines = lines.map((children) => Object.assign(new Token('inline', '', 0), { children, content: '' }));
    const tied = inlines.map((inline, k) => tie(inline, first + k));
    const chunked = chunk(tied);
    out.push(html(Token, `<ul class="snippet-notes" data-snippet="${id}">\n`), ...inlines.flatMap((inline, k) => [
      html(Token, `<li${chunked.has(k) ? ` id="${noteId(k)}"` : ''}${tied[k].attrs.map(([name, value]) => ` ${name}="${value}"`).join('')}>`), inline, html(Token, '</li>\n'),
    ]), html(Token, '</ul>\n'));
    i = notes.end;
  }
  return { tokens: out, warnings };
}

const NUMBER = /^[~≈<>≤≥+−-]?\d[\d.,_]*\s?(?:%|ms|s|m|h|kb|mb|gb|b|x|bytes?)?$/i;
const IDS = /^[A-Z]{1,3}-?\d{1,4}(?:\s*,\s*[A-Z]{1,3}-?\d{1,4})*$/;
const WORD_VALUE = /^[\p{L}\p{N}_-]{1,12}$/u;

/** Whether a table cell holds one short value: a code span or path, a status badge, a number, ids, or one short word. */
export function isShortCell(children) {
  const parts = children.filter((c) => !(c.type === 'text' && !c.content.trim()));
  if (parts.length === 1 && parts[0].type === 'code_inline') return true;
  if (parts[0]?.type === 'html_inline' && parts[0].content.startsWith('<span class="badge')) {
    return parts.length === 1 || (parts.length === 2 && parts[1].type === 'text' && WORD_VALUE.test(parts[1].content.trim()));
  }
  if (parts.length !== 1 || parts[0].type !== 'text') return false;
  const text = parts[0].content.trim();
  return NUMBER.test(text) || IDS.test(text) || WORD_VALUE.test(text);
}

/**
 * Each table's columns whose every filled body cell is a short value, recorded on its `table_open` as `meta.short`
 * (1-based), so the page can keep those columns to their content and give the prose columns the rest.
 */
export function shortColumns(tokens) {
  tokens.forEach((t, i) => {
    if (t.type !== 'table_open') return;
    const close = closeOf(tokens, i);
    const cells = new Map();
    let col = 0;
    for (let k = i; k < close; k += 1) {
      if (tokens[k].type === 'tr_open') col = 0;
      if (tokens[k].type !== 'td_open') continue;
      col += 1;
      const inline = tokens[k + 1];
      const children = inline.type === 'inline' ? inline.children ?? [] : [];
      if (children.some((c) => c.type !== 'text' || c.content.trim())) cells.set(col, [...(cells.get(col) ?? []), isShortCell(children)]);
    }
    const short = [...cells].filter(([, shorts]) => shorts.every(Boolean)).map(([c]) => c).sort((a, b) => a - b);
    if (short.length) t.meta = { ...t.meta, short };
  });
  return tokens;
}

// A paragraph opening with one of these labels is set apart: the trust line and the answer as callouts, a figure's
// per-edge evidence as a note.
const LABELS = [['Trust:', 'trust'], ['Answer:', 'answer'], ['Edges:', 'edges']];

/** Paragraphs whose text starts with a label get that label's class. */
export function labelParagraphs(tokens) {
  tokens.forEach((t, i) => {
    if (t.type !== 'paragraph_open' || tokens[i - 1]?.type === 'list_item_open') return;
    const text = plainText(tokens[i + 1]).trimStart();
    const label = LABELS.find(([word]) => text.startsWith(word));
    if (label) t.attrJoin('class', label[1]);
  });
  return tokens;
}
