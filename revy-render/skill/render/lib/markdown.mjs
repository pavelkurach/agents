// markdown-it configured for plans: marker colouring, status badges, auto-monospace,
// path links, highlighted fences, wide scroll boxes. Raw HTML in markdown is disabled.
import MarkdownIt from 'markdown-it';
import footnote from 'markdown-it-footnote';
import { segmentProse } from './autocode.mjs';
import { COMPONENT_LANGS, renderComponent } from './components/index.mjs';
import { DIAGRAM_LANGS, renderDiagram } from './diagram.mjs';
import { FIGURE_LANGS, figureWarnings, sizeFigure } from './figure.mjs';
import { escapeHtml } from './escape.mjs';
import { commaLists } from './lists.mjs';
import { markChildren } from './marks.mjs';
import { markerOf, nearMarkerOf, statusOf } from './markers.mjs';
import { diffRowId } from './blocks.mjs';
import { classifySpan, locationOf, pathOfSpan, resolveSpan } from './paths.mjs';
import { prFilesUrl } from './refs.mjs';

/**
 * What every render pass carries. `line` locates text that has no source map of its own (component fields);
 * `inComponent` drops the "new file" chip a component's kind already states; `inLink` renders paths unlinked,
 * because a link cannot hold another link.
 * @typedef {{ ctx: object, line?: number, inComponent?: boolean, inLink?: boolean }} Env
 */

const PLAIN_LANGS = new Set(['text', 'txt', 'plain', 'plaintext']);
const FILE_STATUS_BULLET = /^(added|modified|removed)\s/;
const PATH_SHAPED = /^[^\s]*[./][^\s]*$/;
export const FILE_ICON = '<svg class="ficon" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 1.5h5L12.5 5v9.5h-8.5z M9 1.5V5h3.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>';

export const LOCAL_ONLY = '<span class="chip chip-local">local only</span>';
/** The `local only` marker; it opens the evidence entry citing the file when there is one. */
export function localOnly(ctx, rel) {
  const id = ctx.evidenceFor?.(rel);
  return id ? `<a class="chip chip-local" href="#ev-${id}">local only</a>` : LOCAL_ONLY;
}
export const LOCAL_DIFFERS = '<span class="chip chip-differs">local differs</span>';

const lineOf = (token) => (token.meta?.line ?? (token.map ? token.map[0] + 1 : undefined));

export function createMarkdown() {
  const md = new MarkdownIt({ html: false, linkify: false, typographer: false }).enable('table').use(footnote);
  md.core.ruler.push('revy', revyCoreRule);
  md.core.ruler.push('revy-marks', marksCoreRule);
  md.renderer.rules.code_inline = renderCodeInline;
  md.renderer.rules.fence = renderFence;
  md.renderer.rules.image = renderImage;
  md.renderer.rules.table_open = (tokens, idx) => {
    const short = tokens[idx].meta?.short;
    return `<div class="scroll wide table-wrap"${short ? ` data-short="${short.join(' ')}"` : ''}><table>\n`;
  };
  md.renderer.rules.table_close = () => '</table></div>\n';
  // The page draws its own rule between sections; the plugin's <hr> would double it.
  md.renderer.rules.footnote_block_open = () => '<section class="footnotes" aria-label="Footnotes">\n<ol class="footnotes-list">\n';
  return md;
}

/** Plain text of an inline token (code spans keep their text, no backticks). */
export const plainText = (inline) =>
  (inline.children ?? []).map((c) => (c.type === 'softbreak' ? ' ' : c.content)).join('');

function revyCoreRule(state) {
  let lastLine;
  state.tokens.forEach((tok, i) => {
    if (tok.map && !state.inlineMode) lastLine = tok.map[0] + 1;
    if (tok.type !== 'inline') return;
    // Table cells carry no map of their own, so they take their row's; component text takes its fence's.
    const line = lastLine ?? state.env.line;
    const mk = (type, content, meta) => {
      const t = new state.Token(type, type === 'code_inline' ? 'code' : '', 0);
      t.content = content;
      t.meta = { line, ...meta };
      return t;
    };
    let children = tok.children ?? [];
    children.forEach((c) => (c.meta = { line, ...c.meta }));
    const { ctx } = state.env;
    children = applyMarkers(state.tokens, i, children, mk, (text) => ctx.warn(text, line), ctx.fact);
    tok.children = autocode(children, mk);
    if (state.tokens[i - 1]?.type === 'heading_open' || state.env.ledger) return;
    const listed = commaLists(tok.children, mk);
    if (listed.items) ctx.warn(`comma-joined list of ${listed.items} items rendered as a list — write one item per line`, line);
    tok.children = listed.children;
  });
}

/**
 * Text rendered after the document is parsed (component fields, evidence entries) gets the same terms and chips as
 * the document; the document itself is marked once its glossary is known (plan.mjs).
 */
function marksCoreRule(state) {
  const marks = state.env.ctx?.marks;
  if (!marks || !state.inlineMode || state.env.inLink) return;
  const mk = (type, content, meta) => Object.assign(new state.Token(type, type === 'code_inline' ? 'code' : '', 0), { content, meta });
  state.tokens.forEach((tok) => {
    if (tok.type === 'inline') tok.children = markChildren(tok.children ?? [], marks, mk);
  });
}

/** Paths written as code spans in these inline children, without their `::symbol` or line suffix. */
const pathsIn = (children) =>
  children.filter((c) => c.type === 'code_inline' && PATH_SHAPED.test(c.content)).map((c) => pathOfSpan(c.content));

/** Paths in the table row holding token `i`: a status cell counts the file its row names, once. */
function rowPaths(tokens, i) {
  const open = tokens.slice(0, i).findLastIndex((t) => t.type === 'tr_open');
  const close = tokens.findIndex((t, j) => j > i && t.type === 'tr_close');
  return tokens.slice(open, close).filter((t) => t.type === 'inline').flatMap((t) => pathsIn(t.children ?? []));
}

const badgeTokens = (status, children, mk) => [
  mk('html_inline', `<span class="${status.cls}">${escapeHtml(status.word)}</span>`),
  ...(status.rest ? [mk('text', status.rest)] : []),
  ...children.slice(1),
];

function applyMarkers(tokens, i, children, mk, warn, fact) {
  const first = children[0];
  if (first?.type !== 'text') return children;
  const prev = tokens[i - 1]?.type;
  if (prev === 'td_open' || prev === 'th_open') {
    const status = statusOf(first.content);
    if (!status) return children;
    const paths = rowPaths(tokens, i);
    fact(status.word, paths.length ? paths : undefined);
    const row = tokens.slice(0, i).findLast((t) => t.type === 'tr_open');
    if (row && !(row.attrGet('class') ?? '').includes('row-k-')) row.attrJoin('class', `row-k-${status.kind}`);
    return badgeTokens(status, children, mk);
  }
  const item = prev === 'paragraph_open' && tokens[i - 2]?.type === 'list_item_open' ? tokens[i - 2] : undefined;
  if (item && FILE_STATUS_BULLET.test(first.content)) {
    const status = statusOf(first.content);
    const paths = pathsIn(children);
    fact(status.word, paths.length ? paths : undefined);
    item.attrJoin('class', `status-item row-k-${status.kind}`);
    return badgeTokens(status, children, mk);
  }
  const marker = markerOf(first.content);
  if (!marker && prev === 'paragraph_open') {
    const near = nearMarkerOf(first.content);
    if (near) warn(near);
  }
  if (!marker || prev !== 'paragraph_open') return children;
  fact(marker.label);
  (item ?? tokens[i - 1]).attrJoin('class', marker.cls);
  const rest = first.content.slice(marker.label.length);
  return [
    mk('html_inline', `<span class="marker-label">${escapeHtml(marker.label)}</span>`),
    ...(rest ? [mk('text', rest)] : []),
    ...children.slice(1),
  ];
}

function autocode(children, mk) {
  let linkDepth = 0;
  return children.flatMap((c) => {
    if (c.type === 'link_open') linkDepth += 1;
    if (c.type === 'link_close') linkDepth -= 1;
    if (linkDepth > 0) {
      if (c.type === 'code_inline') c.meta = { ...c.meta, inLink: true };
      return [c];
    }
    if (c.type !== 'text') return [c];
    return segmentProse(c.content).map((s) =>
      s.kind === 'code' ? mk('code_inline', s.text, { auto: true }) : mk('text', s.text),
    );
  });
}

/** @param {Env} env */
function renderCodeInline(tokens, idx, _opts, env) {
  const tok = tokens[idx];
  // A long identifier or path may break after `::` or `/`, never inside a name.
  const text = escapeHtml(tok.content).replace(/(::|\/)(?=.)/g, '$1<wbr>');
  if (tok.meta?.inLink || env.inLink) return `<code class="code">${text}</code>`;
  const result = resolveSpan(tok.content, env.ctx);
  const where = lineOf(tok) ?? env.line;
  switch (result.kind) {
    case 'file':
      result.warnings.forEach((w) => env.ctx.warn(w, where));
      return `<a class="path" href="${escapeHtml(result.href)}">${FILE_ICON}<code>${text}</code></a>${result.differs ? LOCAL_DIFFERS : ''}`;
    case 'local':
      return `<span class="path path-local">${FILE_ICON}<code>${text}</code>${localOnly(env.ctx, result.rel)}</span>`;
    case 'new':
      return `<span class="path path-new">${FILE_ICON}<code>${text}</code>${env.inComponent ? '' : '<span class="chip chip-new">new file</span>'}</span>`;
    case 'missing':
      result.warnings.forEach((w) => env.ctx.warn(w, where, true));
      return `<span class="path path-nolink">${FILE_ICON}<code>${text}</code></span>`;
    default:
      return jsonText(tok.content) ? `<code class="code code-json">${inlineJson(tok.content, env.ctx)}</code>` : `<code class="code">${text}</code>`;
  }
}

/** Whether `text` is a JSON object or array (what a reader would call a JSON value worth highlighting). */
export function jsonText(text) {
  if (!/^\s*[[{]/.test(text)) return false;
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

/** One line of JSON highlighted, without the block around it. */
const inlineJson = (text, ctx) => /<span class="line">([^\n]*)<\/span><\/code>/.exec(ctx.highlighter.highlight(text, 'json'))[1];

/**
 * An image is embedded only as a data URI; any other source would leave the page (a network fetch, or a file that
 * does not travel with it), so it becomes a link and a warning.
 * @param {Env} env
 */
function renderImage(tokens, idx, opts, env, self) {
  const tok = tokens[idx];
  const src = tok.attrGet('src');
  const alt = self.renderInlineAsText(tok.children, opts, env);
  if (src.startsWith('data:')) return `<img src="${escapeHtml(src)}" alt="${escapeHtml(alt)}">`;
  env.ctx.warn(`image not embedded, linked instead (the page stays self-contained): ${src}; draw it as a figure or a component`, lineOf(tok) ?? env.line);
  return `<a class="image-link" href="${escapeHtml(src)}">${escapeHtml(alt || src)}</a>`;
}

const infoOf = (tok) => {
  const [lang = '', ...rest] = tok.info.trim().split(/\s+/);
  return { lang: lang.toLowerCase(), caption: rest.join(' ').replace(/^(["'])(.*)\1$/, '$2') };
};

/** @param {Env} env */
function renderFence(tokens, idx, _opts, env) {
  const tok = tokens[idx];
  const { lang, caption } = infoOf(tok);
  const where = lineOf(tok);
  const body = where + 1;
  const { ctx } = env;
  const report = (ws) => ws.forEach((w) => (w.unrenderable ? ctx.unrenderable(w.text, w.line) : ctx.warn(w.text, w.line)));
  const cap = caption ? `<figcaption>${escapeHtml(caption)}</figcaption>` : '';
  if (FIGURE_LANGS.has(lang)) {
    report(figureWarnings(lang, tok.content, body));
    return `<figure class="figure figure-free wide">${cap}${lang === 'svg-figure' ? sizeFigure(tok.content) : tok.content}</figure>\n`;
  }
  if (COMPONENT_LANGS.has(lang)) {
    const { html, warnings } = renderComponent(lang, tok.content, body, ctx.componentIo(body));
    report(warnings);
    return `${html}\n`;
  }
  if (DIAGRAM_LANGS.has(lang)) {
    const { html, warnings } = renderDiagram(lang, tok.content, { id: `fig${where}`, caption, line: body, target: ctx.nodeTarget });
    report(warnings);
    return `${html}\n`;
  }
  if (tok.meta?.diff) return diffFigure(tok.meta.diff, env, where);
  const label = lang ? `<span class="lang">${escapeHtml(lang)}</span>` : '';
  const snippet = tok.meta?.snippet;
  let code;
  if (ctx.highlighter.known.has(lang)) code = ctx.highlighter.highlight(tok.content, lang);
  else {
    if (!lang) ctx.warn('code block has no language; shown plain', where);
    else if (!PLAIN_LANGS.has(lang)) ctx.warn(`unknown code language "${lang}"; shown plain`, where);
    const body = snippet ? tok.content.replace(/\n$/, '').split('\n').map((l) => `<span class="line">${escapeHtml(l)}</span>`).join('\n') : escapeHtml(tok.content);
    code = `<pre class="plain"><code>${body}</code></pre>`;
  }
  if (snippet) return snippetFigure(tok, snippet, label, code, env, where);
  const block = `<div class="scroll wide codeblock">${label}${code}</div>`;
  return caption ? `<figure class="figure codefig wide">${cap}${block}</figure>\n` : `${block}\n`;
}

/**
 * A snippet: lines numbered from its location's first line, each line a link target its notes point at, the lines of
 * each chunk wrapped as one focusable group described by its note, and the location cited as a link to those lines.
 * @param {Env} env
 */
function snippetFigure(tok, { id, start, end, chunks }, label, code, env, where) {
  const [, open, body, close] = /^([^]*?<code>)([^]*?)(<\/code>[^]*)$/.exec(code);
  const lines = body.replace(/\n<span class="line"><\/span>$/, '').split('\n')
    .map((line, k) => line.replace('<span class="line">', `<span class="line" id="${id}-L${start + k}" data-line="${start + k}">`));
  const numbered = `${open}${wrapChunks(lines, chunks)}${close}`;
  const at = locationOf(tok.info);
  // The words after a location are inline markdown, so an evidence id there becomes its chip.
  const rest = at?.rest.replace(/^(["'])(.*)\1$/, '$2');
  const caption = at ? [citation(at, env, where), rest && env.ctx.componentIo(where).inline(rest)].filter(Boolean).join(' ') : escapeHtml(infoOf(tok).caption);
  const cap = caption ? `<figcaption>${caption}</figcaption>` : '';
  return `<figure class="figure codefig snippet wide" id="${id}">${cap}<div class="scroll wide codeblock" style="--ln:${String(end).length + 1}ch">${label}${numbered}</div></figure>\n`;
}

/**
 * Code lines joined, the lines `first`..`last` of each chunk inside one group that names its lines and is described by
 * its note.
 * @param {string[]} lines
 * @param {{ first: number, last: number, name: string, note: string, alt: boolean }[]} chunks in line order
 */
function wrapChunks(lines, chunks) {
  const opens = new Map(chunks.map((c) => [c.first, c]));
  const closes = new Set(chunks.map((c) => c.last));
  return lines.map((line, k) => {
    const c = opens.get(k);
    const head = c ? `<span class="chunk${c.alt ? ' alt' : ''}" tabindex="0" role="group" aria-label="${c.name}" aria-describedby="${c.note}" data-note="${c.note}">` : '';
    return `${head}${line}${closes.has(k) ? '</span>' : ''}`;
  }).join('\n');
}

/**
 * A located diff: each hunk header a row, each code line highlighted in the language of its path with its base and
 * head numbers and its sign in the gutter, the rows of each chunk wrapped as one group described by its note, and the
 * path cited as a link to the file (or to the PR's files view when the info names a PR).
 * @param {{ id: string, at: { span: string, rel: string, pr?: string, rest: string }, diff: import('./diff.mjs').Diff,
 *   lang: string, chunks: object[] }} located
 * @param {Env} env
 */
function diffFigure({ id, at, diff, lang, chunks }, env, where) {
  const { ctx } = env;
  const code = diff.rows.filter((r) => r.kind !== 'hunk');
  const known = ctx.highlighter.known.has(lang);
  const highlighted = known ? ctx.highlighter.highlight(code.map((r) => r.text).join('\n'), lang) : '';
  const [, open = '<pre class="plain"><code>', inner = '', close = '</code></pre>'] = /^([^]*?<code>)([^]*?)(<\/code>[^]*)$/.exec(highlighted) ?? [];
  const texts = known ? inner.split('\n').map((l) => l.replace(/^<span class="line">|<\/span>$/g, '')) : code.map((r) => escapeHtml(r.text));
  const width = String(Math.max(...code.map((r) => Math.max(r.base ?? 0, r.head ?? 0)))).length;
  const pad = (n) => String(n ?? '').padStart(width);
  const SIGN = { add: '+', del: '−', context: ' ' };
  let k = 0;
  const rows = diff.rows.map((r) => {
    if (r.kind === 'hunk') return `<span class="line diff-hunk">${escapeHtml(r.text)}</span>`;
    const cls = r.kind === 'context' ? 'line' : `line diff-${r.kind}`;
    const nums = `${r.head === undefined ? '' : ` data-line="${r.head}"`}${r.base === undefined ? '' : ` data-base="${r.base}"`}`;
    const text = texts[k];
    k += 1;
    return `<span class="${cls}" id="${diffRowId(id, r)}"${nums} data-gutter="${pad(r.base)} ${pad(r.head)} ${SIGN[r.kind]}">${text}</span>`;
  });
  const remote = ctx.marks?.history;
  const cite = at.pr && remote
    ? `<a class="path cite" href="${escapeHtml(prFilesUrl(remote, at.pr))}">${shownPath(at.span)}</a>`
    : citation(at, env, where);
  const caption = [cite, at.rest && ctx.componentIo(where).inline(at.rest)].filter(Boolean).join(' ');
  const label = `<span class="lang">${escapeHtml(known ? lang : 'diff')}</span>`;
  return `<figure class="figure codefig snippet diff wide" id="${id}"><figcaption>${caption}</figcaption><div class="scroll wide codeblock" style="--gut:${2 * width + 3}ch">${label}${open}${wrapChunks(rows, chunks)}${close}</div></figure>\n`;
}

const shownPath = (span) => `${FILE_ICON}<code>${escapeHtml(span).replace(/(::|\/)(?=.)/g, '$1<wbr>')}</code>`;

/** @param {Env} env */
function citation({ span, rel, start, end }, env, where) {
  const shown = shownPath(span);
  const found = classifySpan(start === undefined ? rel : `${rel}:L${start}${end === start ? '' : `-L${end}`}`, env.ctx);
  switch (found.kind) {
    case 'file':
      found.warnings.forEach((w) => env.ctx.warn(w, where));
      return `<a class="path cite" href="${escapeHtml(found.href)}">${shown}</a>${found.differs ? LOCAL_DIFFERS : ''}`;
    case 'local':
      return `<span class="path cite path-local">${shown}${LOCAL_ONLY}</span>`;
    case 'missing':
      found.warnings.forEach((w) => env.ctx.warn(w, where, true));
      return `<span class="path cite path-nolink">${shown}</span>`;
    default:
      return `<span class="path cite path-nolink">${shown}</span>`;
  }
}
