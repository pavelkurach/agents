// Pure: is a render copy its source plus inserted figures? The copy may add figure, component and diagram fences
// and restyle a source Mermaid diagram (caption, kind classes, edge ids); every other line must be the source's.
import { COMPONENT_LANGS } from './components/index.mjs';
import { DIAGRAM_LANGS } from './diagram.mjs';
import { FIGURE_LANGS } from './figure.mjs';

const INSERTABLE = new Set([...COMPONENT_LANGS, ...DIAGRAM_LANGS, ...FIGURE_LANGS]);
const SEMANTIC_RESTYLE = [/^\s*(?:classDef|style|linkStyle)\b/, /^\s*class\s+[\w,-]+\s+\w+\s*;?\s*$/];

/**
 * @typedef {{ n: number, text: string }} Line
 * @typedef {{ kind: 'line', n: number, text: string }} TextItem
 * @typedef {{ kind: 'fence', n: number, text: string, lang: string, body: Line[] }} FenceItem
 * @typedef {{ copyLine: number, copyText: string | undefined, sourceLine: number, sourceText: string | undefined }} Difference
 */

/** Non-blank lines and fenced blocks, with 1-based line numbers. @returns {(TextItem | FenceItem)[]} */
function itemsOf(text) {
  const lines = text.split('\n').map((t, i) => ({ n: i + 1, text: t.trimEnd() }));
  const items = [];
  for (let i = 0; i < lines.length; i += 1) {
    const open = /^ {0,3}(`{3,}|~{3,})\s*([^\s`]*)/.exec(lines[i].text);
    if (open) {
      const close = new RegExp(`^ {0,3}${open[1][0] === '`' ? '`' : '~'}{${open[1].length},}\\s*$`);
      const end = lines.findIndex((l, j) => j > i && close.test(l.text));
      const last = end === -1 ? lines.length : end;
      items.push({ kind: 'fence', n: lines[i].n, text: lines[i].text, lang: open[2].toLowerCase(), body: lines.slice(i + 1, last) });
      i = last;
    } else if (lines[i].text.trim()) items.push({ kind: 'line', ...lines[i] });
  }
  return items;
}

/** A Mermaid body without what restyling may add: kind classes, edge ids, class and style lines. */
const mermaidCore = (body) => body
  .filter((l) => l.text.trim() && !SEMANTIC_RESTYLE.some((re) => re.test(l.text)))
  .map((l) => ({ ...l, core: l.text.replace(/:::[\w-]+/g, '').replace(/\b[A-Za-z_][\w-]*@(?=[-=.~<ox])/g, '').replace(/\s+/g, ' ').trim() }));

/** The first line where two items differ, or undefined when they are the same. @returns {Difference | undefined} */
function compareItems(a, b) {
  const at = (x, y) => ({ copyLine: x?.n ?? a.n, copyText: x?.text, sourceLine: y?.n ?? b.n, sourceText: y?.text });
  if (a.kind !== b.kind || a.lang !== b.lang) return at(a, b);
  if (a.kind === 'line') return a.text === b.text ? undefined : at(a, b);
  if (a.lang === 'mermaid') {
    const [x, y] = [mermaidCore(a.body), mermaidCore(b.body)];
    const k = Array.from({ length: Math.max(x.length, y.length) }, (_, i) => i).find((i) => x[i]?.core !== y[i]?.core);
    return k === undefined ? undefined : at(x[k], y[k]);
  }
  const [x, y] = [[a, ...a.body], [b, ...b.body]];
  const k = Array.from({ length: Math.max(x.length, y.length) }, (_, i) => i).find((i) => x[i]?.text !== y[i]?.text);
  return k === undefined ? undefined : at(x[k], y[k]);
}

/** @returns {Difference | undefined} the first place the copy is not the source plus inserted figures */
export function copyDifference(copyText, sourceText) {
  const copy = itemsOf(copyText);
  const source = itemsOf(sourceText);
  const endOf = (text) => text.replace(/\n$/, '').split('\n').length + 1;
  let j = 0;
  for (const a of copy) {
    const b = source[j];
    const paired = b && (b.kind === a.kind && b.lang === a.lang);
    const inserted = a.kind === 'fence' && INSERTABLE.has(a.lang) && !paired;
    if (inserted) continue;
    if (!b) return { copyLine: a.n, copyText: a.text, sourceLine: endOf(sourceText), sourceText: undefined };
    const diff = compareItems(a, b);
    if (diff) return diff;
    j += 1;
  }
  return j < source.length
    ? { copyLine: endOf(copyText), copyText: undefined, sourceLine: source[j].n, sourceText: source[j].text }
    : undefined;
}

/**
 * For a copy that `copyDifference` accepts: maps a copy line to its source line, or to undefined inside an inserted
 * figure (or the blank lines after one), which has no source line.
 * @returns {(copyLine: number) => number | undefined}
 */
export function sourceLineOf(copyText, sourceText) {
  const source = itemsOf(sourceText);
  let j = 0;
  const anchors = itemsOf(copyText).map((a) => {
    const b = source[j];
    const paired = b && b.kind === a.kind && b.lang === a.lang;
    if (a.kind === 'fence' && INSERTABLE.has(a.lang) && !paired) return { from: a.n, source: undefined };
    j += 1;
    return { from: a.n, source: b?.n };
  });
  return (line) => {
    const anchor = anchors.findLast((x) => x.from <= line);
    if (!anchor) return line;
    return anchor.source === undefined ? undefined : anchor.source + (line - anchor.from);
  };
}
