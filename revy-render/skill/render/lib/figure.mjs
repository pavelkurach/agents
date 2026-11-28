// Free-form figures (`svg-figure`, `html-figure`) pass through verbatim; this is their kit and their lint.

export const FIGURE_LANGS = new Set(['html-figure', 'svg-figure']);

/**
 * Markers and patterns a hand-drawn figure references by id: `marker-end="url(#fig-arrow)"`. They paint with
 * `context-stroke`, so an arrowhead takes its edge's colour in both themes. Emitted once per page.
 */
export const FIGURE_DEFS = `<svg class="fig-defs" width="0" height="0" aria-hidden="true" focusable="false"><defs>
<marker id="fig-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="10" markerHeight="10" orient="auto-start-reverse"><path d="M0 0L10 5L0 10z" fill="context-stroke"/></marker>
<marker id="fig-arrow-open" viewBox="0 0 10 10" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="10" markerHeight="10" orient="auto-start-reverse"><path d="M1 1L9 5L1 9" fill="none" stroke="context-stroke" stroke-width="1.5"/></marker>
<marker id="fig-dot" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="7" markerHeight="7"><circle cx="5" cy="5" r="4" fill="context-stroke"/></marker>
<marker id="fig-cross" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto"><path d="M1 1L9 9M9 1L1 9" stroke="context-stroke" stroke-width="1.8"/></marker>
<pattern id="fig-hatch" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="8" height="8" style="fill:var(--surface)"/><line x1="0" y1="0" x2="0" y2="8" style="stroke:var(--line-strong)" stroke-width="3"/></pattern>
</defs></svg>`;

const RAW_COLOUR = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![\w-])|\b(?:rgba?|hsla?)\(\s*\d/g;

// An attribute or CSS reference to an http(s) or protocol-relative URL, and CSS @import.
const REMOTE = /\b(?:src|href|xlink:href|srcset|poster|data)\s*=\s*["']?(?:https?:)?\/\/[^"'\s>]*["']?|url\(\s*["']?(?:https?:)?\/\/[^)]*\)|@import\b[^;]*/gi;

/** Raw colours, an unlabelled <svg>, scripts and network references. @returns {{text:string, line:number}[]} */
export function figureWarnings(lang, markup, firstLine) {
  const lineAt = (index) => firstLine + markup.slice(0, index).split('\n').length - 1;
  const colours = [...markup.matchAll(RAW_COLOUR)]
    .filter((m) => !/(?:url\(\s*|href\s*=\s*["']?)$/i.test(markup.slice(Math.max(0, m.index - 12), m.index)))
    .map((m) => ({
      text: `${lang}: raw colour ${m[0].startsWith('#') ? m[0] : `${m[0]}…)`} ignores dark mode; use a token (var(--ink), var(--new) …) or a kit class (.fig-node.k-new)`,
      line: lineAt(m.index),
    }));
  const unlabelled = [...markup.matchAll(/<svg\b[^>]*>/g)]
    .filter((m) => !/\baria-(?:label|labelledby|hidden)\s*=/.test(m[0]))
    .map((m) => ({ text: `${lang}: <svg> has no aria-label; add one stating what the figure shows`, line: lineAt(m.index) }));
  const scripts = [...markup.matchAll(/<script\b/gi)]
    .map((m) => ({ text: `${lang}: <script> runs in the reader's browser; draw the figure without one`, line: lineAt(m.index) }));
  const remote = [...markup.matchAll(REMOTE)]
    .map((m) => ({ text: `${lang}: ${m[0]} loads from the network; the page must work offline`, line: lineAt(m.index) }));
  return [...colours, ...unlabelled, ...scripts, ...remote];
}


const MIN_TEXT = 12;
const KIT_LABEL = 14;

/**
 * Pure: the figure's root <svg> drawn at its viewBox size, shrinking with the column but never so far that its
 * smallest text drops under 12 px (the frame scrolls instead). Markup without a viewBox is left as written.
 */
export function sizeFigure(markup) {
  return markup.replace(/<svg\b[^>]*>/, (tag) => {
    const vb = /\bviewBox\s*=\s*["']\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)/.exec(tag);
    if (!vb || /\bwidth\s*=/.test(tag.replace(/\bstroke-width|style\s*=\s*"[^"]*"/g, ''))) return tag;
    const sizes = [...markup.matchAll(/font-size\s*[:=]\s*["']?([\d.]+)/g)].map((m) => Number(m[1]));
    const smallest = Math.min(KIT_LABEL, ...sizes);
    const width = Number(vb[1]);
    const style = `width:${width}px;max-width:100%;min-width:${Math.ceil((width * MIN_TEXT) / smallest)}px`;
    const existing = /\sstyle\s*=\s*"([^"]*)"/.exec(tag);
    return existing ? tag.replace(existing[0], ` style="${style};${existing[1]}"`) : tag.replace(/<svg\b/, `<svg style="${style}"`);
  });
}
