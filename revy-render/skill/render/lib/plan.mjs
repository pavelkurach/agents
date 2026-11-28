// Plan markdown + look + repos -> {html, warnings}. Deterministic for fixed inputs.
import { readFileSync } from 'node:fs';
import { escapeHtml } from './escape.mjs';
import { FIGURE_DEFS } from './figure.mjs';
import { FONT_CSS } from './fonts.mjs';
import { orderSections, parseLook } from './look.mjs';
import { createMarkdown, FILE_ICON, LOCAL_DIFFERS, localOnly, plainText, jsonText } from './markdown.mjs';
import { prepareHighlighter } from './highlight.mjs';
import { addedPathsOf, classifySpan, pathOfSpan, resolveSpan } from './paths.mjs';
import { digestOf, foldTails, glossaries, labelParagraphs, snippetNotes, shortColumns } from './blocks.mjs';
import { anchorGlossary, glossaryTerms, markTokens, termIn, termPattern } from './marks.mjs';
import { anchorFigures, definedIds, definedPattern } from './refs.mjs';

const CSS = readFileSync(new URL('../theme.css', import.meta.url), 'utf8');
// The page's only script: theme, navigation, tips and figure interaction, all progressive enhancement. In <head>, so a
// remembered theme applies before the first paint.
const PAGE_JS = readFileSync(new URL('../page.js', import.meta.url), 'utf8');

/** The facts strip: which counted words appear in which group, in display order, with their kind. */
const FACT_GROUPS = [
  { title: 'Files', words: [['added', 'new'], ['modified', 'changed'], ['removed', 'removed']] },
  { title: 'Decisions', words: [['one-way', 'oneway'], ['reversible', 'context'], ['internal', 'context']] },
  { title: 'Risks', words: [['Risk:', 'risk', 'risk', 'risks'], ['Blind spot:', 'failure', 'blind spot', 'blind spots'], ['Accepted:', 'accepted', 'accepted', 'accepted'], ['Mitigation:', 'new', 'mitigation', 'mitigations']] },
];

/**
 * Pure: counts from the document -> the facts strip; a component's counts replace prose counts of the same group.
 * @param {{ text: Map<string, number>, component: Map<string, number> }} counts
 */
export function factsHtml(counts) {
  const groups = FACT_GROUPS.map((g) => {
    const from = g.words.some(([w]) => counts.component.has(w)) ? counts.component : counts.text;
    const chips = g.words.filter(([w]) => from.get(w)).map(([w, kind, one = w, many = w]) => {
      const n = from.get(w);
      return `<span class="fact k-${kind}"><b>${n}</b> ${escapeHtml(n === 1 ? one : many)}</span>`;
    });
    return chips.length ? `<div><dt>${g.title}</dt><dd>${chips.join('')}</dd></div>` : '';
  }).join('');
  return groups ? `<dl class="facts">${groups}</dl>` : '';
}

const digest = (body) => {
  const line = digestOf(body);
  return line ? `<span class="digest">${line}</span>` : '';
};

const sizes = (map) => new Map([...map].map(([word, ids]) => [word, ids.size]));

const slugify = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'section';

/** Split block tokens into a preamble and one group per level-0 h2. */
function splitSections(tokens) {
  const preamble = [];
  const sections = [];
  tokens.forEach((t, i) => {
    if (t.type === 'heading_open' && t.tag === 'h2' && t.level === 0) {
      sections.push({ title: plainText(tokens[i + 1]).trim(), tokens: [] });
    }
    (sections.at(-1)?.tokens ?? preamble).push(t);
  });
  return { preamble, sections };
}

function emphasiseSnippets(tokens, snippets) {
  const hit = new Set();
  tokens.forEach((t, i) => {
    if (t.type !== 'inline') return;
    snippets.filter((s) => t.content.includes(s)).forEach((s) => hit.add(s));
    if (!snippets.some((s) => t.content.includes(s))) return;
    const prev = tokens[i - 1];
    const target = prev.type === 'paragraph_open' && tokens[i - 2]?.type === 'list_item_open' ? tokens[i - 2] : prev;
    target.attrJoin('class', 'emph');
  });
  return hit;
}

/** The one place a component reaches the page: inline markdown, code spans and tree rows, located at its fence. */
function componentIo(md, ctx, line) {
  const env = { ctx, line, inComponent: true };
  const { Token } = new md.core.State('', md, {});
  const codeToken = (text) => Object.assign(new Token('code_inline', 'code', 0), { content: text });
  const warnAt = (w) => ctx.warn(w, line);
  return {
    inline: (text, { inLink = false } = {}) => md.renderInline(text, { ...env, inLink }),
    code: (text) => md.renderer.renderInline([codeToken(text)], md.options, env),
    // A component's code value: highlighted as JSON when it parses as JSON or its content type says JSON.
    codeBlock: (text, cls, type) => (jsonText(text) || /json/i.test(type ?? '')
      ? ctx.highlighter.highlight(text, 'json').replace('<pre class="shiki', `<pre class="${cls} shiki`)
      : `<pre class="${cls}"><code>${escapeHtml(text)}</code></pre>`),
    fact: ctx.fact,
    // A tree row shows the file's name and links it like a code span; an added file has nothing to link to yet.
    // A tree's root is often the repository's own name, so a file not found under it is looked up without it.
    file: (full, status, label, inRoot) => {
      const shown = `${FILE_ICON}<code>${escapeHtml(label)}</code>`;
      if (status === 'added') return `<span class="path path-new" title="${escapeHtml(full)}">${shown}</span>`;
      const underRoot = classifySpan(full, ctx);
      const found = underRoot.kind === 'missing' && inRoot ? classifySpan(inRoot, ctx) : underRoot;
      if (found.kind === 'file') {
        found.warnings.forEach(warnAt);
        return `<a class="path" href="${escapeHtml(found.href)}" title="${escapeHtml(full)}">${shown}</a>${found.differs ? LOCAL_DIFFERS : ''}`;
      }
      if (found.kind === 'local') return `<span class="path path-local" title="${escapeHtml(full)}">${shown}${localOnly(ctx, found.rel)}</span>`;
      if (found.kind === 'missing' && ctx.repos.length) warnAt(`tree path not found in any --repo: ${full}`);
      return `<span class="path path-nolink" title="${escapeHtml(full)}">${shown}</span>`;
    },
  };
}

/** A section body split at its level-0 h3s: what precedes the first, then one group per h3 with its title. */
function splitEntries(body) {
  const lead = [];
  const entries = [];
  body.forEach((t, i) => {
    if (t.type === 'heading_open' && t.tag === 'h3' && t.level === 0) entries.push({ title: plainText(body[i + 1]).trim(), tokens: [] });
    (entries.at(-1)?.tokens ?? lead).push(t);
  });
  return { lead, entries };
}

/** Footnote tokens the plugin appends after the document's last block, split off so they render after every section. */
function splitFootnotes(tokens) {
  const at = tokens.findIndex((t) => t.type === 'footnote_block_open');
  return at === -1 ? { body: tokens, footnotes: [] } : { body: tokens.slice(0, at), footnotes: tokens.slice(at) };
}

/** A section's or entry's name in the navigation: its title, with the code it names shown as code. */
function navLabel({ title, tokens }) {
  const children = tokens?.[1]?.children ?? [];
  if (!children.some((c) => c.type === 'code_inline')) return escapeHtml(title);
  const textOf = (c) => (c.type === 'softbreak' ? ' ' : c.type === 'html_inline' ? c.content.replace(/<[^>]+>/g, '') : c.content);
  return children.map((c) => (c.type === 'code_inline' ? `<code class="code">${escapeHtml(c.content)}</code>` : escapeHtml(textOf(c)))).join('').trim();
}

/**
 * @param {{ planText: string, planName: string, locate?: (line: number) => string, lookJson?: object, repos: object[],
 *   link?: import('./paths.mjs').LinkBase, evidence?: { name: string, entries: Map<string, import('./evidence.mjs').EvidenceEntry> } }} input
 *   `locate` turns a line of `planText` into the place a warning cites (default `<planName>:<line>`); `link` is `--link`;
 *   `evidence` is the parsed `--evidence` ledger and its file name.
 * @returns {Promise<{ html: string, warnings: string[], errors: string[] }>} `errors`: the warnings about blocks that
 *   could not be drawn and are shown as source.
 */
export async function renderPlan({ planText, planName, locate = (n) => `${planName}:${n}`, lookJson, repos, link, evidence }) {
  const look = parseLook(lookJson ?? {});
  const warnings = [];
  const seen = new Set();
  const errors = [];
  const warn = (what, where, once = false) => {
    const loc = typeof where === 'number' ? locate(where) : (where ?? planName);
    const line = `${what} — ${loc}`;
    const key = once ? what : line;
    if (!seen.has(key)) {
      seen.add(key);
      warnings.push(line);
    }
    return line;
  };
  // A block drawn as its source instead of as itself: still a warning, and an error to `--lint`.
  const unrenderable = (what, where) => errors.push(warn(what, where));
  repos.filter((r) => r.git?.remote && r.git.guessedRef).forEach((r) => warn(
    'HEAD is on no remote branch and origin/HEAD is unset, so links point at branch "main"; push HEAD or run `git remote set-head origin -a`',
    `--repo ${r.root}`,
  ));
  // Each counted word holds the things it was said of (a file path, or one occurrence), so a file named twice counts once.
  const counts = { text: new Map(), component: new Map() };
  /** @type {(word: string, ids?: string[], from?: 'text' | 'component') => void} */
  const fact = (word, ids = [Symbol(word)], from = 'text') =>
    counts[from].set(word, new Set([...(counts[from].get(word) ?? []), ...ids]));
  const md = createMarkdown();
  const { Token } = new md.core.State('', md, {});
  const parsed = splitFootnotes(md.parse(planText, { ctx: { warn, fact } }));
  const codeNotes = snippetNotes(glossaries(parsed.body, Token), Token);
  codeNotes.warnings.forEach((w) => warn(w.text, w.line));
  const tokens = shortColumns(labelParagraphs(foldTails(codeNotes.tokens, look.tail, Token)));
  const { footnotes } = parsed;
  const highlighter = await prepareHighlighter(
    [...tokens, ...footnotes].filter((t) => t.type === 'fence').map((t) => t.meta?.diff?.lang ?? t.info.trim().split(/\s+/)[0].toLowerCase()),
  );
  const cited = citedSpans(evidence?.entries);
  const ctx = { unrenderable, repos, link, citations: [...cited.keys()], evidenceFor: (rel) => cited.get(rel), addedPaths: addedPathsOf(planText), warn, fact, highlighter, componentIo: (line) => componentIo(md, ctx, line) };
  const env = { ctx };
  const terms = glossaryTerms(tokens);
  const ids = definedIds(tokens);
  ids.warnings.forEach((w) => warn(w.text, w.line));
  ctx.marks = {
    terms,
    match: termPattern(terms),
    evidence: evidence?.entries,
    linked: (code) => resolveSpan(code, ctx).kind !== 'code',
    missing: (id, line) => warn(`evidence ${id} has no entry in ${evidence.name}; shown as plain text`, line, true),
    used: new Set(),
    history: link?.remote ?? repos[0]?.git?.remote,
    defined: ids.defined,
    definedMatch: definedPattern(ids.defined),
    anchors: [],
  };
  markTokens([...tokens, ...footnotes], ctx.marks, Token);
  ctx.nodeTarget = nodeTarget(ctx, evidence?.entries, cited);

  const { preamble, sections } = splitSections(tokens);
  const titles = new Set(sections.map((s) => s.title));
  [['order', look.order], ['fold', look.fold], ['collapse', look.collapse]].forEach(([key, list]) =>
    list.filter((t) => !titles.has(t)).forEach((t) => warn(`look.${key} names unknown section "${t}"`, 'look file')),
  );
  const snippets = look.emphasis.filter((e) => !titles.has(e));
  const hit = emphasiseSnippets(tokens, snippets);
  snippets.filter((s) => !hit.has(s)).forEach((s) => warn(`look.emphasis matches no section or text "${s}"`, 'look file'));

  const ordered = orderSections(sections, look.order);
  const sectionIds = new Set();
  const uniqueId = (title) => {
    let id = slugify(title);
    for (let n = 2; sectionIds.has(id); n += 1) id = `${slugify(title)}-${n}`;
    sectionIds.add(id);
    return id;
  };
  const placed = ordered.map((s) => ({ ...s, id: uniqueId(s.title) }));

  const render = (ts) => md.renderer.render(ts, md.options, env);
  // A collapsed section's ### entries each become a closed disclosure, listed under the section in the navigation.
  const entriesOf = (s) => (look.collapse.includes(s.title) ? splitEntries(s.tokens.slice(3)) : undefined);
  const withEntries = placed.map((s) => {
    const split = entriesOf(s);
    return { ...s, split, entries: split?.entries.map((e) => ({ ...e, id: uniqueId(e.title) })) ?? [] };
  });
  const bodyHtml = (s, body) => (s.split ? render(s.split.lead) + s.entries.map((e) => {
    const [open, inline, close, ...rest] = e.tokens;
    return `<details class="entry" id="${e.id}"><summary>${render([open, inline, close]).trimEnd()}</summary>\n${render(rest)}</details>\n`;
  }).join('') : render(body));
  const sectionHtml = withEntries.map((s) => {
    const [open, inline, close, ...body] = s.tokens;
    const heading = render([open, inline, close]);
    const cls = `section${look.emphasis.includes(s.title) ? ' emph' : ''}`;
    return look.fold.includes(s.title)
      ? `<details class="${cls} folded" id="${s.id}"><summary>${heading.trimEnd()}${digest(body)}</summary>\n${bodyHtml(s, body)}</details>`
      : `<section class="${cls}" id="${s.id}">\n${heading}${bodyHtml(s, body)}</section>`;
  });

  const h1 = preamble.findIndex((t) => t.type === 'heading_open' && t.tag === 'h1');
  const title = h1 === -1 ? planName : plainText(preamble[h1 + 1]).trim();
  const heading = h1 === -1 ? `<h1>${escapeHtml(title)}</h1>` : render(preamble.slice(h1, h1 + 3));
  const lead = render(h1 === -1 ? preamble : [...preamble.slice(0, h1), ...preamble.slice(h1 + 3)]);
  // The ledger is notes, not the document: its lines are linted by ledger rules (no comma runs made lists) and what
  // it is warned about is kept apart, so it cannot drown the document's own warnings.
  const ledgerWarnings = new Set(evidence?.warnings ?? []);
  const appendix = evidenceHtml(evidence, (text, line) => md.renderInline(text, {
    ctx: { ...ctx, warn: (w) => ledgerWarnings.add(`${w} — ${evidence.name}:${line}`) },
    line,
    ledger: true,
  }));
  const navItems = [...withEntries, ...(appendix ? [{ id: 'evidence', title: 'Evidence', entries: [] }] : [])];
  const sub = (entries) => (entries.length ? `<ol class="toc-sub">${entries.map((e) => `<li><a href="#${e.id}">${navLabel(e)}</a></li>`).join('')}</ol>` : '');
  const toc = navItems.length < 2 ? '' : `<nav class="toc" aria-label="Sections"><p class="toc-title">Sections</p><ol>${navItems
    .map((s) => `<li><a href="#${s.id}">${navLabel(s)}</a>${sub(s.entries)}</li>`).join('')}</ol></nav>`;
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<meta name="color-scheme" content="light dark">
<title>${escapeHtml(title)}</title>
<style>
${FONT_CSS}
${CSS}</style>
<script>${PAGE_JS}</script>
</head>
<body>
${FIGURE_DEFS}
${look.watermark ? `<div class="ribbon" aria-hidden="true">${escapeHtml(look.watermark)}</div>\n` : ''}<div class="page">
<header class="masthead intro">
<p class="eyebrow">${escapeHtml(planName)} · ${placed.length} section${placed.length === 1 ? '' : 's'}</p>
${heading}<div class="lead">
${lead}</div>
${factsHtml({ text: sizes(counts.text), component: sizes(counts.component) })}
</header>
${toc}
<main>
${sectionHtml.join('\n')}
${appendix}${render(footnotes)}</main>
</div>
</body>
</html>
`;
  const page = anchorFigures(anchorGlossary(html, ctx.marks), ctx.marks.anchors.map((a) => a.id));
  const onPage = new Set([...page.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
  ctx.marks.anchors.filter((a) => !onPage.has(a.id)).forEach((a) => warn(`link #${a.id} matches no id on the page`, a.line));
  const ledgerLines = [...ledgerWarnings];
  const grouped = ledgerLines.length
    ? [`evidence: ${ledgerLines.length} warning${ledgerLines.length === 1 ? '' : 's'} about ${evidence.name} itself`, ...ledgerLines.map((w) => `evidence: ${w}`)]
    : [];
  return { html: page, warnings: [...warnings, ...grouped], errors };
}

/**
 * Code spans the evidence cites, and the file each names, mapped to the first entry citing them.
 * @returns {Map<string, string>}
 */
function citedSpans(entries) {
  const cited = new Map();
  [...(entries?.values() ?? [])].forEach((e) => [...`${e.citation} ${e.what}`.matchAll(/`([^`]+)`/g)].forEach(([, span]) => {
    [span, pathOfSpan(span)].forEach((key) => { if (!cited.has(key)) cited.set(key, e.id); });
  }));
  return cited;
}

/**
 * What a diagram node leads to. A label that is a glossary term opens its entry; one that names a symbol or path
 * opens its code (or, absent at the link commit, the evidence citing it); else a term inside the label decides.
 * @returns {import('./diagram.mjs').NodeTarget}
 */
function nodeTarget(ctx, entries, cited) {
  const evidenceTip = (...spans) => {
    const id = spans.map((s) => cited.get(s)).find(Boolean);
    const e = id && entries.get(id);
    return e ? ` · ${id} — ${e.what.replace(/`([^`]*)`/g, '$1')}` : '';
  };
  const term = (label, exact) => {
    const t = termIn(ctx.marks, label, exact);
    return t && { kind: 'term', href: t.href, tip: t.def };
  };
  return (label) => {
    const exact = term(label, true);
    if (exact) return exact;
    if (!/\s/.test(label)) {
      const found = resolveSpan(label, ctx);
      if (found.kind === 'file') return { kind: 'code', href: found.href, tip: `${found.cite ?? label}${evidenceTip(label, found.cite && pathOfSpan(found.cite))}` };
      const id = found.kind === 'local' && cited.get(found.rel);
      if (id) return { kind: 'code', href: `#ev-${id}`, tip: `${found.rel} · local only${evidenceTip(found.rel)}` };
    }
    return term(label, false);
  };
}

/**
 * The evidence ledger as a folded appendix after the last section, one entry per id, each its own link target.
 * @param {{ name: string, entries: Map<string, import('./evidence.mjs').EvidenceEntry> } | undefined} evidence
 * @param {(text: string, line: number) => string} inline
 */
// A bare citation that is one place (`a.md::Tier table:L18-L22`) is one span, its label's spaces included.
const PLACE = /^[\w.-]+(?:\/[\w.-]+)*\.\w+(?:::[^`—]*?)?(?::L\d+(?:-L?\d+)?)?$/;
const asSpan = (citation) => (PLACE.test(citation) ? `\`${citation}\`` : citation);

function evidenceHtml(evidence, inline) {
  if (!evidence?.entries.size) return '';
  const items = [...evidence.entries.values()].map((e) => `<li id="ev-${e.id}"><span class="ev-id">${e.id}</span>`
    + `${e.markers.length ? `<span class="ev-via">${e.markers.map(escapeHtml).join(' + ')}</span>` : ''}`
    + `${e.citation ? `<span class="ev-cite">${inline(asSpan(e.citation), e.line)}</span>` : ''}`
    + `<span class="ev-what">${inline(e.what, e.line)}</span>`
    + `${e.backs ? `<span class="ev-backs"><span class="ev-label">backs</span> ${inline(e.backs, e.line)}</span>` : ''}</li>`);
  const n = items.length;
  return `<details class="section folded evidence" id="evidence"><summary><h2>Evidence</h2><span class="digest">${n} entr${n === 1 ? 'y' : 'ies'} from ${escapeHtml(evidence.name)}</span></summary>\n`
    + `<ol class="evidence-list">\n${items.join('\n')}\n</ol></details>\n`;
}
