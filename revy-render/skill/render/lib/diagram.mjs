// Mermaid fences become inline SVG at build time through Pele (MIT). Colours come from page tokens through Pele's
// --pele-* variables and the semantic classes, so light/dark switch with no re-render.
import { parse as peleParse, PeleError, render as pele } from 'pele';
import { segmentProse } from './autocode.mjs';
import { escapeHtml } from './escape.mjs';
import { measurer, MONO_STACK, SANS_STACK } from './fonts.mjs';
import { outcomeOf } from './outcome.mjs';
import { SEMANTIC, legendHtml } from './semantic.mjs';
import { codePills, groupEnd, okMarks } from './svglabels.mjs';
import { pinnedOrder, walkOrder } from './walk.mjs';

export const DIAGRAM_LANGS = new Set([
  'mermaid', 'plantuml', 'd2', 'dot', 'graphviz', 'flowchart', 'sequence', 'seq', 'gantt',
]);

const MIN_TEXT = 12;
const MAX_FONT = 20;
// Wide fits the page's figure box at 1440; narrow fits a 390 px phone inside its gutters and frame.
const WIDE = 1080;
const NARROW = 330;
// Re-layouts tried, in order, when the wide layout overflows the column: wrap long labels and close up the ranks
// (keeps the reading direction), then turn LR into TD.
const WRAPPED = { config: { flowchart: { wrappingWidth: 110, rankSpacing: 30 } } };
const TURNED = { directionBreakpoint: Infinity };
const RELAYOUTS = [WRAPPED, TURNED, { ...WRAPPED, ...TURNED }];

/**
 * What a node's label leads to: a glossary term's entry or a symbol's code, and the tip shown for it.
 * @typedef {(label: string) => { kind: 'term' | 'code', href: string, tip: string } | undefined} NodeTarget
 */

/**
 * @param {string} lang
 * @param {string} source the fence body
 * @param {{ id:string, caption:string, line:number, target?: NodeTarget }} figure  line = 1-based plan line of the
 *   fence's first body line; `target` makes nodes naming a term or a symbol links
 * @returns {{ html:string, warnings:{text:string, line:number}[] }}
 */
export function renderDiagram(lang, source, figure) {
  if (lang !== 'mermaid') {
    return {
      html: sourceFigure(lang, source, figure.caption),
      warnings: [{ text: `no diagram engine for "${lang}"; shown as source (write it as mermaid)`, line: figure.line - 1, unrenderable: true }],
    };
  }
  const warnings = [...rawColourWarnings(source, figure.line), ...rectColourWarnings(source, figure.line)];
  let wide;
  let narrow;
  try {
    const drawn = sequenceSemicolons(monoLabels(source));
    wide = fitted(drawn, `${figure.id}w`);
    narrow = draw(drawn, NARROW, `${figure.id}n`);
  } catch (e) {
    if (!(e instanceof PeleError)) throw e;
    const at = e.line ? figure.line + e.line - 1 : figure.line;
    const text = source.split('\n')[e.line - 1]?.trim();
    const why = e.message.split('\n').map((l) => l.trim())
      .filter((l) => l && !/^-*\^$/.test(l) && l !== text && !/^Parse error on line \d+:?$/.test(l)).join(' ');
    return {
      html: sourceFigure(lang, source, figure.caption, true),
      warnings: [...warnings, { text: `Mermaid ${e.code} error${text ? ` in "${text}"` : ''}: ${why}; shown as source`, line: at, unrenderable: true }],
    };
  }
  const edgeClasses = edgeClassesOf(source);
  const ends = drawnEnds(edgeEnds(source, wide.type), wide, figure.line, warnings);
  const flow = flowOutcomes(source, wide.type);
  const label = figure.caption || firstNodeLabel(wide.svg) || `${wide.mermaidType} diagram`;
  // Pele's narrow journey lets task text run out of its boxes; the wide one scrolls instead.
  const both = wide.type !== 'journey' && narrow.width < wide.width - 1;
  const finish = (r, cls) => {
    const marked = okMarks(decorate(r, label, edgeClasses, flow, cls), r.fontSize);
    return { svg: linkNodes(withEnds(codePills(marked.svg, r.fontSize), ends), figure.target), unmarked: marked.unmarked };
  };
  const drawnWide = finish(wide, both ? ' is-wide' : '');
  const svgWide = drawnWide.svg;
  const svgNarrow = both ? finish(narrow, ' is-narrow').svg : '';
  drawnWide.unmarked.forEach((id) => warnings.push({ text: `ok node "${id}" has no outline to put its check mark on; it is marked by colour only`, line: figure.line - 1 }));
  const used = SEMANTIC.map((k) => k.cls).filter((cls) => new RegExp(`class="pele-(?:node|edge|rect)[^"]*\\b${cls}\\b`).test(svgWide));
  const legend = legendHtml(used);
  const steps = stepsOf(source, wide.type, ends, svgWide, figure.line, warnings);
  return {
    html: `<figure class="figure diagram wide" data-kind="mermaid-${escapeHtml(wide.mermaidType)}"${steps.length > 1 ? ` data-steps="${escapeHtml(JSON.stringify(steps))}"` : ''}>`
      + `${figure.caption ? `<figcaption>${escapeHtml(figure.caption)}</figcaption>` : ''}`
      + `<div class="scroll diagram-frame">${svgWide}${svgNarrow}</div>${legend}</figure>`,
    warnings,
  };
}

const NOT_LABELS = /^\s*(?:%%|classDef\b|class\b|style\b|linkStyle\b|click\b)/;
// Text after the first single colon of a line whose arrow comes first: a sequence message, a state transition, a note.
const MESSAGE = /^([^:\n]*?(?:-{1,2}>{1,2}|-{1,2}[x)]|==>|-\.->|--|\bnote\b)[^:\n]*:(?!:))(.*)$/i;
// The innermost text of a node shape or edge label, unquoted: `[…]`, `(…)`, `{…}`, `|…|`.
const SHAPE = /([[({|])([^[\](){}|"<>\n]+)([\])}|])/g;

const codeTags = (text) => (text.includes('<') ? text : text
  .replace(/`([^`]+)`/g, '<code>$1</code>')
  .split(/(<code>[^<]*<\/code>)/)
  .map((part) => (part.startsWith('<code>') ? part : segmentProse(part).map((x) => (x.kind === 'code' ? `<code>${x.text}</code>` : x.text)).join('')))
  .join(''));

/**
 * Pure: label text that is code (a backticked span, or code-shaped like `s3::put_object`) wrapped in `<code>`, which
 * Pele measures and draws in the mono face. Node ids, keywords and quoted labels are left as written.
 */
export function monoLabels(source) {
  return source.split('\n').map((line) => {
    if (NOT_LABELS.test(line)) return line;
    const message = MESSAGE.exec(line);
    if (message) return `${message[1]}${codeTags(message[2])}`;
    return line.replace(SHAPE, (_, open, text, close) => `${open}${codeTags(text)}${close}`);
  }).join('\n');
}

/**
 * Pele draws some labels smaller than its base size; the base grows until the smallest label is 12 px.
 * @returns {import('pele').RenderResult & { fontSize: number }}
 */
function draw(source, maxWidth, idPrefix, layout = {}) {
  for (let fontSize = 14; ; fontSize += 1) {
    const r = pele(source, { fontSize, maxWidth, idPrefix, measurer, fontFamily: SANS_STACK, fontFamilyMono: MONO_STACK, ...layout });
    const smallest = Math.min(...[...r.svg.matchAll(/font-size(?:="|:\s*)([\d.]+)/g)].map((m) => Number(m[1])));
    if (fontSize >= MAX_FONT || smallest >= MIN_TEXT) return { ...r, fontSize };
  }
}

/** The authored layout when it fits the column; else the first re-layout that fits, else the narrowest one drawn. */
function fitted(source, idPrefix) {
  const authored = draw(source, WIDE, idPrefix);
  if (authored.width <= WIDE) return authored;
  const tried = RELAYOUTS.map((layout) => draw(source, WIDE, idPrefix, layout));
  return tried.find((r) => r.width <= WIDE) ?? [authored, ...tried].reduce((a, b) => (b.width < a.width ? b : a));
}

// A sequence message drawn with a cross head (`-x`, `--x`) is a failed call, so it is drawn as a failure.
// Natural width as min-width: the SVG never scales below its drawn size, so text keeps its 12 px; the frame scrolls.
// Edges and end nodes the author left unclassed take the outcome their label names, marked `inferred`.
function decorate(r, label, edgeClasses, flow, cls) {
  let svg = r.svg
    .replace(' role="graphics-document document"', ` role="img" aria-label="${escapeHtml(label)}"`)
    .replace('class="pele ', `class="pele${cls} `)
    .replace('style="', `style="min-width:${Math.ceil(r.width)}px;`);
  const failed = new Set([...svg.matchAll(/<g class="pele-edge pele-message" data-id="([^"]+)">(?:(?!<\/g>).)*?<path class="pele-marker" d="M[^"]*M[^"]*"\/>/gs)].map((m) => decodeEntities(m[1])));
  const authored = new Map([...[...failed].map((id) => [id, 'failure']), ...edgeClasses]);
  const inferred = new Map([...edgeLabelOutcomes(svg), ...flow.edges].filter(([id]) => !authored.has(id) && !flow.classedEdges.has(id))
    .map(([id, outcome]) => [id, `${outcome} inferred`]));
  new Map([...inferred, ...authored]).forEach((semantic, id) => {
    const attr = `data-id="${escapeHtml(id)}"`;
    svg = svg
      .replace(/class="(pele-edge(?:-label)?(?: pele-[a-z-]+)*)" (data-id="[^"]+")/g, (m, c, a) => (a === attr ? `class="${c} ${semantic}" ${a}` : m));
  });
  flow.nodes.forEach((outcome, id) => {
    const attr = `data-id="${escapeHtml(id)}"`;
    svg = svg.replace(/class="(pele-node(?: pele-[a-z-]+)*)" (data-id="[^"]+")/g, (m, c, a) => (a === attr ? `class="${c} ${outcome} inferred" ${a}` : m));
  });
  // `rect focus` reaches the SVG as a fill named after the class; it becomes the class.
  return svg.replace(/<rect class="pele-rect" ([^>]*?)style="([^"]*?)fill:([a-z]+);"/g, (m, attrs, style, name) => (
    SEMANTIC_NAMES.has(name) ? `<rect class="pele-rect ${name}" ${attrs}style="${style}"` : m));
}

const decodeEntities = (s) => s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');

function firstNodeLabel(svg) {
  const m = /class="pele-node[^"]*"[^>]*>(?:(?!<\/g>).)*?<text[^>]*>((?:(?!<\/text>).)*)<\/text>/s.exec(svg);
  const text = m ? decodeEntities(m[1].replace(/<[^>]+>/g, '')).trim() : '';
  return text || undefined;
}

const SEMANTIC_NAMES = new Set(SEMANTIC.map((k) => k.cls));

/** `;` ends a statement in a sequence diagram, so one inside message text is written as Mermaid's entity for it. */
function sequenceSemicolons(source) {
  if (!/^\s*sequenceDiagram\b/.test(source)) return source;
  return source.split('\n').map((line) => {
    const m = /^([^:]*(?:-{1,2}>{1,2}|-{1,2}[x)]|\bnote\b[^:]*)[^:]*:)(.*)$/i.exec(line);
    return m ? `${m[1]}${m[2].replace(/;/g, '#59;')}` : line;
  }).join('\n');
}

function rectColourWarnings(source, firstLine) {
  return source.split('\n').flatMap((text, i) => {
    const m = /^\s*rect\s+(.+?)\s*$/.exec(text);
    if (!m || SEMANTIC_NAMES.has(m[1]) || hueOf(m[1]) === undefined) return [];
    return [{ text: `Mermaid rect sets a raw colour (${m[1]}), which ignores dark mode; write \`rect focus\` to mark the span the figure is about`, line: firstLine + i }];
  });
}

/**
 * Edge id -> the node ids it joins, in declaration order: a flowchart's from Pele's own parse, a state diagram's
 * transitions numbered as Pele numbers them; empty for other diagram types.
 * @returns {Map<string, { from: string, to: string }>}
 */
function edgeEnds(source, type) {
  if (type === 'flowchart') return new Map(peleParse(source).edges.map((e) => [e.id, { from: e.start, to: e.end }]));
  if (type === 'state') return stateEnds(peleParse(source).rootDoc);
  return new Map();
}

// Pele draws transition k as `edge<k>`, counting in one depth-first pass over the statements in which a note takes a
// number too; its parse does not expose the ids, so they are counted the same way here and checked against the drawing.
function stateEnds(doc) {
  const ends = new Map();
  let k = 0;
  const walk = (stmts) => stmts.forEach((s) => {
    if (s.stmt === 'relation') ends.set(`edge${k++}`, { from: s.state1.id, to: s.state2.id });
    if (s.stmt !== 'state') return;
    if (s.note) k += 1;
    if (s.doc) walk(s.doc);
  });
  walk(doc);
  return ends;
}

/** The ends, when they name exactly the transitions drawn; else none, with a warning that tracing and stepping are off. */
function drawnEnds(ends, r, line, warnings) {
  if (r.type !== 'state') return ends;
  const drawn = [...r.svg.matchAll(/<g class="pele-edge pele-transition[^"]*" data-id="([^"]+)"/g)].map((m) => decodeEntities(m[1]));
  if (drawn.length === ends.size && drawn.every((id) => ends.has(id))) return ends;
  warnings.push({ text: `state diagram: ${drawn.length} transitions drawn but ${ends.size} counted, so they cannot be matched to their states; tracing and step-through are off for this figure`, line: line - 1 });
  return new Map();
}

/**
 * The order the page steps a flowchart or state diagram through: the author's `%% steps: A, B, C` when given (unknown
 * nodes skipped with a warning), else walked from the graph. Edge kinds come from the drawn classes, so a step-through
 * follows the colours the reader sees.
 * @returns {import('./walk.mjs').Step[]}
 */
function stepsOf(source, type, ends, svg, firstLine, warnings) {
  const lines = source.split('\n');
  const at = lines.findIndex((l) => /^\s*%%\s*steps\s*:/i.test(l));
  if (type !== 'flowchart' && type !== 'state') {
    if (at >= 0) warnings.push({ text: `Mermaid %% steps only orders flowcharts and state diagrams; a ${type} diagram steps through its messages in order`, line: firstLine + at });
    return [];
  }
  const classesOf = (kind) => new Map([...svg.matchAll(new RegExp(`<g class="pele-${kind}((?: [^"]*)?)" data-id="([^"]+)"`, 'g'))]
    .map((m) => [decodeEntities(m[2]), m[1].split(' ')]));
  const nodes = classesOf('(?:node|cluster)');
  const edgeClasses = classesOf('edge');
  const edges = [...ends].map(([id, { from, to }]) => {
    const cls = edgeClasses.get(id) ?? [];
    const failure = cls.includes('failure') || nodes.get(to)?.includes('failure');
    return { id, from, to, kind: failure ? 'failure' : cls.includes('ok') ? 'ok' : 'plain' };
  });
  // A composite state's own start is declared before the diagram's; the walk begins at the diagram's.
  if (at < 0) return walkOrder(type === 'state' ? [...edges.filter((e) => e.from === 'root_start'), ...edges.filter((e) => e.from !== 'root_start')] : edges);
  const named = lines[at].replace(/^\s*%%\s*steps\s*:/i, '').split(',').map((s) => s.trim()).filter(Boolean);
  named.filter((id) => !nodes.has(id)).forEach((id) => warnings.push({
    text: `Mermaid %% steps names "${id}", which is no node of this diagram (nodes: ${[...nodes.keys()].join(', ')}); it is skipped`,
    line: firstLine + at,
  }));
  return pinnedOrder(edges, named.filter((id) => nodes.has(id)));
}

/** Each edge and edge label names the nodes it joins, so the page can trace a node's paths. */
function withEnds(svg, ends) {
  if (!ends.size) return svg;
  return svg.replace(/(<g class="pele-edge(?:-label)?[^"]*" data-id="([^"]+)")>/g, (m, open, id) => {
    const e = ends.get(decodeEntities(id));
    return e ? `${open} data-from="${escapeHtml(e.from)}" data-to="${escapeHtml(e.to)}">` : m;
  });
}

/**
 * Edge id -> the outcome its drawn label names, for every diagram type that labels edges.
 * @returns {[string, import('./outcome.mjs').Outcome][]}
 */
function edgeLabelOutcomes(svg) {
  return [...svg.matchAll(/<g class="pele-edge-label[^"]*" data-id="([^"]+)"[^>]*>((?:(?!<\/g>).)*)<\/g>/gs)]
    .map((m) => [decodeEntities(m[1]), outcomeOf(decodeEntities(m[2].replace(/<[^>]+>/g, ' ')))])
    .filter(([, outcome]) => outcome);
}

/**
 * From a flowchart's parse: edges the author classed, edges drawn with a cross head (a failure unless their label
 * names success), and the outcome each unclassed end node (no outgoing edge) names. Empty for other diagram types.
 * @returns {{ classedEdges: Set<string>, edges: Map<string, import('./outcome.mjs').Outcome>, nodes: Map<string, import('./outcome.mjs').Outcome> }}
 */
function flowOutcomes(source, type) {
  if (type !== 'flowchart') return { classedEdges: new Set(), edges: new Map(), nodes: new Map() };
  const { nodes, edges } = peleParse(source);
  const starts = new Set(edges.map((e) => e.start));
  const crossed = edges.filter((e) => /cross/.test(e.type) && outcomeOf(e.text ?? '') !== 'ok');
  return {
    classedEdges: new Set(edges.filter((e) => e.classes.length).map((e) => e.id)),
    edges: new Map(crossed.map((e) => [e.id, 'failure'])),
    nodes: new Map([...nodes.values()].filter((n) => !n.classes.length && !starts.has(n.id))
      .map((n) => [n.id, outcomeOf(n.text ?? '')]).filter(([, outcome]) => outcome)),
  };
}

/** A node whose label names a glossary term or a symbol becomes a link, its tip as the link's title. */
function linkNodes(svg, target) {
  if (!target) return svg;
  const node = /<g class="pele-node[ "]/g;
  let out = '';
  let at = 0;
  for (let m = node.exec(svg); m; node.lastIndex = at, m = node.exec(svg)) {
    const start = m.index;
    const end = groupEnd(svg, start);
    const group = svg.slice(start, end);
    const label = [...group.matchAll(/<text[^>]*>((?:(?!<\/text>).)*)<\/text>/gs)]
      .map((m) => decodeEntities(m[1].replace(/<[^>]+>/g, ' '))).join(' ').replace(/\s+/g, ' ').trim();
    const hit = label ? target(label) : undefined;
    out += svg.slice(at, start) + (hit
      ? `<a class="node-link node-${hit.kind}" href="${escapeHtml(hit.href)}"><title>${escapeHtml(hit.tip)}</title>${group}</a>`
      : group);
    at = end;
  }
  return out + svg.slice(at);
}

/**
 * Pele puts no class on an edge, but keeps a declared edge id (`A e1@--> B`) as the edge's data-id. A Mermaid
 * `class e1 failure` line therefore names the edge, and the renderer adds the class.
 */
function edgeClassesOf(source) {
  const ids = new Set([...source.matchAll(/(?:^|[\s;])([A-Za-z_][\w-]*)@(?=[-=.~<ox])/gm)].map((m) => m[1]));
  const classes = new Map();
  for (const m of source.matchAll(/^\s*class\s+([\w,-]+)\s+(\w+)\s*;?\s*$/gm)) {
    if (!SEMANTIC_NAMES.has(m[2])) continue;
    m[1].split(',').filter((id) => ids.has(id)).forEach((id) => classes.set(id, m[2]));
  }
  return classes;
}

// --- raw colours ------------------------------------------------------------------------------------------

const NAMED = {
  red: 0, crimson: 348, tomato: 9, orange: 39, darkorange: 33, gold: 51, yellow: 60, green: 120, lime: 120,
  limegreen: 120, seagreen: 146, teal: 180, cyan: 180, blue: 240, navy: 240, royalblue: 225, steelblue: 207,
  purple: 300, violet: 300, magenta: 300, indigo: 275, pink: 350, brown: 0, salmon: 6,
};
const GREYS = new Set(['grey', 'gray', 'silver', 'black', 'white', 'lightgrey', 'lightgray', 'darkgrey', 'darkgray', 'gainsboro', 'whitesmoke']);

/** Hue in degrees, or null for a grey; undefined when `value` is not a colour. */
function hueOf(value) {
  const v = value.trim().toLowerCase();
  if (GREYS.has(v)) return null;
  if (v in NAMED) return NAMED[v];
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})(?:[0-9a-f]{2})?$/.exec(v);
  const rgbFn = /^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/.exec(v);
  const hsl = /^hsla?\(\s*([\d.]+)/.exec(v);
  if (hsl) return Number(hsl[1]) % 360;
  let rgb;
  if (hex) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join('') : hex[1];
    rgb = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
  } else if (rgbFn) rgb = rgbFn.slice(1, 4).map(Number);
  else return undefined;
  const [r, g, b] = rgb.map((c) => c / 255);
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  if (max - min < 0.08) return null;
  const d = max - min;
  const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return (h * 60 + 360) % 360;
}

function semanticFor(hue) {
  if (hue === null) return 'context';
  if (hue < 15 || hue >= 330) return 'failure';
  if (hue < 70) return 'risk';
  if (hue < 170) return 'new';
  if (hue < 255) return 'changed';
  return 'oneway';
}

const COLOUR_PROPS = /\b(fill|stroke|color|background(?:-color)?)\s*:\s*([^,;\s]+(?:\([^)]*\))?)/g;

function rawColourWarnings(source, firstLine) {
  return source.split('\n').flatMap((text, i) => {
    const m = /^\s*(classDef|style|linkStyle)\s+(\S+)/.exec(text);
    if (!m) return [];
    const colours = [...text.matchAll(COLOUR_PROPS)].filter((c) => hueOf(c[2]) !== undefined);
    if (!colours.length) return [];
    const [, kw, target] = m;
    const suggest = SEMANTIC_NAMES.has(target) && kw === 'classDef' ? target : semanticFor(hueOf(colours[0][2]));
    const how = kw === 'linkStyle'
      ? `give the edge an id (\`A e1@--> B\`) and write \`class e1 ${suggest}\``
      : `use the semantic class \`:::${suggest}\``;
    return [{
      text: `Mermaid ${kw} ${target} sets a raw colour (${colours.map((c) => `${c[1]}:${c[2]}`).join(', ')}), which ignores dark mode; ${how}`,
      line: firstLine + i,
    }];
  });
}

function sourceFigure(lang, source, caption, failed = false) {
  return `<figure class="figure diagram wide${failed ? ' diagram-failed' : ''}" data-kind="source">`
    + `${caption ? `<figcaption>${escapeHtml(caption)}</figcaption>` : ''}`
    + `<div class="scroll"><pre class="diagram-src" data-lang="${escapeHtml(lang)}"><code>${escapeHtml(source)}</code></pre></div></figure>`;
}
