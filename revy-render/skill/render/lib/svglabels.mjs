// Pure SVG passes over a drawn diagram's labels: code runs set on a grey pill, so code reads apart from words the
// way inline code does in prose, and the check mark that is the `ok` kind's second signal beside its colour.
import { measurer } from './fonts.mjs';
import { SEMANTIC } from './semantic.mjs';

const MONO = 4;
const BOLD = 1;
// Pill box in em around the mono text's baseline: covers ascenders and descenders, and stays inside the 18 px line
// pitch of a wrapped 14 px label so stacked pills never touch.
const PILL = { padX: 0.3, above: 0.92, height: 1.25, radius: 0.25 };
const NODE_MARK_R = 7;
const LABEL_MARK_R = 6;

const r2 = (n) => Math.round(n * 100) / 100;
const attr = (tag, name) => new RegExp(`\\s${name}="([^"]*)"`).exec(tag)?.[1];
const num = (tag, name) => {
  const v = attr(tag, name);
  return v === undefined ? undefined : Number(v);
};
const decode = (s) => s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
const isMonoFamily = (tag) => attr(tag, 'font-family') === 'var(--_fm)';
const isBoldWeight = (tag) => /^(?:bold|[6-9]00)$/.test(attr(tag, 'font-weight') ?? '');

const SEMANTIC_ALT = SEMANTIC.map((k) => k.cls).join('|');
// The page draws these labels bold (theme.css), so they are measured bold.
const BOLD_NODE = /\bpele-node\b.*\b(?:failure|ok)\b/;
const BOLD_EDGE_LABEL = new RegExp(`\\bpele-edge-label\\b.*\\b(?:${SEMANTIC_ALT})\\b`);

/**
 * @typedef {{ text: string, mono: boolean, bold: boolean }} Run
 * @typedef {{ x: number, y: number, runs: Run[] }} Line
 * @typedef {{ left: number, baseline: number, size: number, runs: (Run & { at: number, width: number })[] }} PlacedLine
 */

/**
 * The lines of one `<text>` element, each a start point and its runs. A tspan carrying `x` starts a line.
 * @param {string} open the `<text …>` tag
 * @param {string} inner its content
 * @returns {Line[]}
 */
function linesOf(open, inner) {
  const mono = isMonoFamily(open);
  const bold = isBoldWeight(open);
  const spans = [...inner.matchAll(/<tspan\b([^>]*)>([^<]*)<\/tspan>/g)];
  if (!spans.length) {
    const x = num(open, 'x');
    const y = num(open, 'y');
    if (x === undefined || y === undefined) throw new Error(`diagram label without a position: ${open}${inner}</text>`);
    return [{ x, y, runs: [{ text: decode(inner), mono, bold }] }];
  }
  /** @type {Line[]} */
  const lines = [];
  spans.forEach(([, tag, text]) => {
    const run = { text: decode(text), mono: mono || isMonoFamily(tag), bold: bold || isBoldWeight(tag) };
    const x = num(tag, 'x');
    if (x === undefined && lines.length) {
      lines[lines.length - 1].runs.push(run);
      return;
    }
    const y = num(tag, 'y') ?? num(open, 'y');
    const lineX = x ?? num(open, 'x');
    if (lineX === undefined || y === undefined) throw new Error(`diagram label line without a position: <tspan${tag}>${text}</tspan>`);
    lines.push({ x: lineX, y, runs: [run] });
  });
  return lines;
}

/**
 * Lines placed by their anchor, each run with its measured start and width.
 * @param {string} open @param {string} inner @param {number} size @param {boolean} bold
 * @returns {PlacedLine[]}
 */
function placed(open, inner, size, bold) {
  const anchor = attr(open, 'text-anchor') ?? 'start';
  return linesOf(open, inner).map((line) => {
    const widths = line.runs.map((r) => measurer.width(r.text, size, (r.mono ? MONO : 0) | (r.bold || bold ? BOLD : 0)));
    const total = widths.reduce((a, b) => a + b, 0);
    const left = anchor === 'middle' ? line.x - total / 2 : anchor === 'end' ? line.x - total : line.x;
    let at = left;
    const runs = line.runs.map((r, i) => {
      const run = { ...r, at, width: widths[i] };
      at += widths[i];
      return run;
    });
    return { left, baseline: line.y, size, runs };
  });
}

/** @param {PlacedLine} line @returns {string} one rect per non-blank mono run */
const pillRects = (line) => line.runs
  .filter((r) => r.mono && r.text.trim())
  .map((r) => {
    const s = line.size;
    return `<rect x="${r2(r.at - PILL.padX * s)}" y="${r2(line.baseline - PILL.above * s)}" width="${r2(r.width + 2 * PILL.padX * s)}" height="${r2(PILL.height * s)}" rx="${r2(PILL.radius * s)}"/>`;
  })
  .join('');

/**
 * Walks the SVG's groups and texts, tracking the font size and classes in effect, and calls `onText` for each text.
 * @param {string} svg
 * @param {number} baseSize the diagram's base font size
 * @param {(t: { open: string, inner: string, size: number, bold: boolean }) => string} onText markup to put before it
 * @returns {string}
 */
function eachText(svg, baseSize, onText) {
  /** @type {{ size: number | undefined, cls: string }[]} */
  const stack = [];
  return svg.replace(/<g\b[^>]*?(\/?)>|<\/g>|<text\b[^>]*>[\s\S]*?<\/text>/g, (m, selfClosing) => {
    if (m === '</g>') {
      stack.pop();
      return m;
    }
    if (m.startsWith('<g')) {
      if (!selfClosing) stack.push({ size: num(m, 'font-size'), cls: attr(m, 'class') ?? '' });
      return m;
    }
    const [, open, inner] = /^(<text\b[^>]*>)([\s\S]*)<\/text>$/.exec(m);
    const size = num(open, 'font-size') ?? [...stack].reverse().find((g) => g.size !== undefined)?.size ?? baseSize;
    const bold = stack.some((g) => BOLD_NODE.test(g.cls) || BOLD_EDGE_LABEL.test(g.cls));
    return onText({ open, inner, size, bold }) + m;
  });
}

/**
 * Every code run in a label gets a rounded grey rect behind it, sized from the measured mono text.
 * @param {string} svg @param {number} baseSize
 * @returns {string}
 */
export function codePills(svg, baseSize) {
  return eachText(svg, baseSize, ({ open, inner, size, bold }) => {
    if (!isMonoFamily(open) && !/<tspan\b[^>]*font-family="var\(--_fm\)"/.test(inner)) return '';
    const rects = placed(open, inner, size, bold).map(pillRects).join('');
    return rects ? `<g class="code-pill">${rects}</g>` : '';
  });
}

const checkMark = (cx, cy, r) => `<g class="ok-mark" aria-hidden="true"><circle cx="${r2(cx)}" cy="${r2(cy)}" r="${r}"/>`
  + `<path d="M${r2(cx - r * 0.45)} ${r2(cy + r * 0.02)}l${r2(r * 0.32)} ${r2(r * 0.34)} ${r2(r * 0.6)}-${r2(r * 0.66)}"/></g>`;

/** Top-right corner of a node's outline, where its check mark sits. */
function cornerOf(group) {
  const shape = /<(rect|circle|ellipse|polygon|path)\b([^>]*)>/.exec(group);
  if (!shape) return undefined;
  const [, kind, attrs] = shape;
  const tag = ` ${attrs}`;
  if (kind === 'rect') return { x: (num(tag, 'x') ?? 0) + (num(tag, 'width') ?? 0), y: num(tag, 'y') ?? 0 };
  if (kind === 'circle' || kind === 'ellipse') {
    const rx = num(tag, 'r') ?? num(tag, 'rx') ?? 0;
    const ry = num(tag, 'r') ?? num(tag, 'ry') ?? 0;
    return { x: (num(tag, 'cx') ?? 0) + rx * Math.SQRT1_2, y: (num(tag, 'cy') ?? 0) - ry * Math.SQRT1_2 };
  }
  const values = (attr(tag, kind === 'polygon' ? 'points' : 'd') ?? '').match(/-?\d*\.?\d+(?:e-?\d+)?/gi)?.map(Number) ?? [];
  const xs = values.filter((_, i) => i % 2 === 0);
  const ys = values.filter((_, i) => i % 2 === 1);
  return xs.length ? { x: Math.max(...xs), y: Math.min(...ys) } : undefined;
}

/** Just left of an edge label: its background rect, or else its first measured line. */
function labelStart(group, baseSize, size) {
  const rect = /<rect\b([^>]*)>/.exec(group);
  if (rect) {
    const tag = ` ${rect[1]}`;
    return { x: (num(tag, 'x') ?? 0) - LABEL_MARK_R + 1, y: (num(tag, 'y') ?? 0) + (num(tag, 'height') ?? 0) / 2 };
  }
  const text = /(<text\b[^>]*>)([\s\S]*?)<\/text>/.exec(group);
  if (!text) return undefined;
  const [line] = placed(text[1], text[2], size ?? baseSize, true);
  return { x: line.left - LABEL_MARK_R - 2, y: line.baseline - 0.35 * line.size };
}

/** The index just past the `</g>` closing the group that opens at `from`. */
export function groupEnd(svg, from) {
  const tag = /<g\b[^>]*>|<\/g>/g;
  tag.lastIndex = from;
  let depth = 0;
  for (let m = tag.exec(svg); m; m = tag.exec(svg)) {
    depth += m[0] === '</g>' ? -1 : 1;
    if (depth === 0) return m.index + m[0].length;
  }
  return svg.length;
}

/**
 * Each node and edge label of the `ok` kind gets a check mark: on the node's top-right corner, at the label's left end.
 * @param {string} svg @param {number} baseSize
 * @returns {{ svg: string, unmarked: string[] }} `unmarked`: data-ids of ok nodes whose outline could not be read
 */
export function okMarks(svg, baseSize) {
  const open = /<g class="(pele-node|pele-edge-label)\b[^"]*\bok\b[^"]*" data-id="([^"]*)"[^>]*>/g;
  const edgeLabelSize = num(/<g class="pele-edge-labels"[^>]*>/.exec(svg)?.[0] ?? '', 'font-size');
  const unmarked = [];
  let out = '';
  let at = 0;
  for (let m = open.exec(svg); m; m = open.exec(svg)) {
    const end = groupEnd(svg, m.index);
    const group = svg.slice(m.index, end);
    const isNode = m[1] === 'pele-node';
    const spot = isNode ? cornerOf(group) : labelStart(group, baseSize, edgeLabelSize);
    if (!spot) unmarked.push(decode(m[2]));
    const mark = spot ? checkMark(spot.x, spot.y, isNode ? NODE_MARK_R : LABEL_MARK_R) : '';
    out += svg.slice(at, end - '</g>'.length) + mark + '</g>';
    at = end;
    open.lastIndex = end;
  }
  return { svg: out + svg.slice(at), unmarked };
}
