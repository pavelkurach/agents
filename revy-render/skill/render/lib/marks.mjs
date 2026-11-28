// Pure token transforms that make prose interactive: glossary terms become links with their definition as a tip,
// evidence ids (`E3`) chips with their citation as a tip, history refs links to the forge, and ids the document
// defines links to their definition. All are plain links without the page script.
import { escapeHtml } from './escape.mjs';
import { plainText } from './markdown.mjs';
import { HISTORY, historyRef, historyUrl, ledgerMentions } from './refs.mjs';

/**
 * @typedef {{ variants: string[], def: string, href: string, id: string, anchor: { content: string }, definition: object }} Term
 *   `anchor` is the glossary entry's opening token (the entry gets its id only when something links to it);
 *   `definition` is its definition's inline token, where the term itself is not linked.
 * @typedef {import('./evidence.mjs').EvidenceEntry} EvidenceEntry
 * @typedef {{ terms: Term[], match: RegExp | undefined, evidence: Map<string, EvidenceEntry> | undefined,
 *   linked: (code: string) => boolean, missing: (id: string, line: number | undefined) => void, used: Set<Term>,
 *   history?: { kind: 'github' | 'gitlab' | 'bitbucket', host: string, project: string }, defined?: Map<string, string>,
 *   definedMatch?: RegExp, anchors?: { id: string, line: number | undefined }[] }} Marks
 *   `history`: the forge history refs link to; `defined`: ids the document defines -> their anchor; `anchors`: every
 *   `#id` link met, for checking once the page is rendered.
 */

const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'term';
const oneLine = (s) => s.replace(/`([^`]*)`/g, '$1').replace(/\s+/g, ' ').trim();
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const EVIDENCE_ID = /\bE\d+\b/g;
const WORD = '[\\p{L}\\p{N}_]';

/** Glossary entries of `glossaries()` output, one per definition-list row; a term "a, b" names two variants. */
export function glossaryTerms(tokens) {
  return tokens.flatMap((t, i) => {
    if (t.meta?.glossary !== 'term') return [];
    const def = tokens[i + 2];
    const term = plainText(t).trim();
    const link = (def.children ?? []).find((c) => c.type === 'link_open' && c.attrGet('href')?.startsWith('#'));
    const id = `g-${slug(term)}`;
    return [{
      variants: term.split(/\s*,\s*/).filter(Boolean),
      def: oneLine(plainText(def)),
      href: link ? link.attrGet('href') : `#${id}`,
      id,
      anchor: tokens[i - 1],
      definition: def,
    }];
  });
}

/** One case-insensitive whole-word pattern for every variant, longest first; a plural `s`/`es` still counts as the word. */
export function termPattern(terms) {
  const variants = [...new Set(terms.flatMap((t) => t.variants))].sort((a, b) => b.length - a.length);
  if (!variants.length) return undefined;
  // A hyphen joining letters makes one word (`rule-index`), so a term next to one is not a whole word.
  return new RegExp(`(?<!${WORD}|${WORD}-)(${variants.map(escapeRe).join('|')})(?:s|es)?(?!${WORD}|-${WORD})`, 'giu');
}

const termOf = (marks, text) => {
  const key = text.toLowerCase();
  return marks.terms.find((t) => t.variants.some((v) => v.toLowerCase() === key))
    ?? marks.terms.find((t) => t.variants.some((v) => key.startsWith(v.toLowerCase()) && /^(?:s|es)$/.test(key.slice(v.length))));
};

/** The term `text` is (exact), or the first term inside it; undefined without one. */
export function termIn(marks, text, exact) {
  if (exact || !marks.match) return termOf(marks, text);
  const m = new RegExp(marks.match.source, 'iu').exec(text);
  return m ? termOf(marks, m[0]) : undefined;
}

const termLink = (marks, term, inner) => {
  marks.used.add(term);
  return `<a class="term" href="${escapeHtml(term.href)}" title="${escapeHtml(term.def)}">${inner}</a>`;
};

/** A chip for an evidence id, or undefined (and a report) when the ledger has no such entry. */
function chip(marks, id, line) {
  const e = marks.evidence.get(id);
  if (!e) {
    marks.missing(id, line);
    return undefined;
  }
  const tip = `${id}${e.citation ? ` · ${oneLine(e.citation)}` : ''} — ${oneLine(e.what)}`;
  return `<a class="ev" href="#ev-${id}" title="${escapeHtml(tip)}">${id}</a>`;
}

/** A history ref linked to the forge, its tip what the ledger says of it or else the bare ref and host. */
function historyLink(marks, text) {
  const ref = historyRef(text);
  const said = marks.evidence ? ledgerMentions(marks.evidence.values(), ref) : [];
  const tip = said.length ? said.map((e) => `${e.id} — ${oneLine(e.what)}`).join('; ') : `${text} · ${marks.history.host}/${marks.history.project}`;
  return `<a class="ref" href="${escapeHtml(historyUrl(marks.history, ref))}" title="${escapeHtml(tip)}">${escapeHtml(text)}</a>`;
}

/** A `#id` link is recorded, to be checked against the rendered page. */
function recordAnchor(open, marks) {
  const href = open.attrGet('href') ?? '';
  if (href.startsWith('#') && href.length > 1) marks.anchors?.push({ id: decodeURIComponent(href.slice(1)), line: open.meta?.line });
}

/** Split `text` at `re`'s matches, mapping each match to HTML (or undefined to leave it as text). */
function splitAt(text, re, toHtml) {
  const out = [];
  let last = 0;
  for (const m of text.matchAll(re)) {
    const html = toHtml(m[0]);
    if (html === undefined) continue;
    if (m.index > last) out.push({ text: text.slice(last, m.index) });
    out.push({ html });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last) });
  return out;
}

/**
 * Inline children with evidence ids chipped, history refs and defined ids linked, glossary terms linked, and `#id`
 * links recorded. Text inside links is left alone; a code span is a term only when the whole span is one
 * (never a word inside it), and never when it is itself a path link.
 * @param {object[]} children
 * @param {Marks} marks
 * @param {(type: string, content: string, meta?: object) => object} mk token factory
 * @param {{ terms?: boolean }} [opts] terms: false in headings, which then link neither terms nor defined ids
 */
export function markChildren(children, marks, mk, { terms = true } = {}) {
  let depth = 0;
  const withTerms = terms && marks.match;
  // Text is split by each pass in turn; a part one pass made into HTML is left to the next.
  const passes = [
    marks.evidence && [EVIDENCE_ID, (id, line) => chip(marks, id, line)],
    marks.history && [HISTORY, (ref) => historyLink(marks, ref)],
    terms && marks.definedMatch && [marks.definedMatch, (id) => `<a class="xref" href="#${marks.defined.get(id)}">${id}</a>`],
    withTerms && [marks.match, (word) => {
      const term = termOf(marks, word);
      return term ? termLink(marks, term, escapeHtml(word)) : undefined;
    }],
  ].filter(Boolean);
  return children.flatMap((c) => {
    if (c.type === 'link_open') {
      recordAnchor(c, marks);
      depth += 1;
    }
    if (c.type === 'link_close') depth -= 1;
    if (depth > 0) return [c];
    if (c.type === 'code_inline' && withTerms && !marks.linked(c.content)) {
      const term = termOf(marks, c.content);
      if (!term) return [c];
      const [open, close] = termLink(marks, term, '\u0000').split('\u0000');
      return [mk('html_inline', open), Object.assign(c, { meta: { ...c.meta, inLink: true } }), mk('html_inline', close)];
    }
    if (c.type !== 'text' || c.meta?.defines) return [c];
    const line = c.meta?.line;
    const parts = passes.reduce((ps, [re, toHtml]) => ps.flatMap((p) => (p.text !== undefined
      ? splitAt(p.text, re, (m) => toHtml(m, line))
      : [p])), [{ text: c.content }]);
    if (parts.length === 1 && parts[0].text !== undefined) return [c];
    return parts.map((p) => (p.html !== undefined ? mk('html_inline', p.html, { line }) : mk('text', p.text, { line })));
  });
}

/**
 * Every inline of the document marked: headings get chips but no terms, and a glossary's own terms get neither.
 * Each glossary entry's opening tag becomes a placeholder that `anchorGlossary` resolves once the page is rendered.
 */
export function markTokens(tokens, marks, Token) {
  const mk = (type, content, meta) => Object.assign(new Token(type, type === 'code_inline' ? 'code' : '', 0), { content, meta });
  tokens.forEach((t, i) => {
    if (t.type !== 'inline' || t.meta?.glossary === 'term') return;
    const own = marks.terms.find((term) => term.definition === t);
    const scoped = own ? { ...marks, terms: marks.terms.filter((term) => term !== own), match: termPattern(marks.terms.filter((term) => term !== own)) } : marks;
    t.children = markChildren(t.children ?? [], scoped, mk, { terms: tokens[i - 1]?.type !== 'heading_open' });
  });
  marks.terms.forEach((term) => {
    term.anchor.content = term.anchor.content.replace('<div>', `\u0001${term.id}\u0001<div>`);
  });
}

/** Rendered page with each glossary entry that something links to given its id; the others stay bare. */
export function anchorGlossary(html, marks) {
  const used = new Set([...marks.used].map((t) => t.id));
  return html.replace(/\u0001([^\u0001]+)\u0001<div>/g, (_, id) => (used.has(id) ? `<div id="${id}">` : '<div>'));
}
