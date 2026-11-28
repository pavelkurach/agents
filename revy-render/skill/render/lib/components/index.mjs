// Built-in components: a fenced block whose language names the component and whose body is YAML, drawn as
// HTML/CSS that reflows and cannot clip. Schemas: references/components.md.
import { escapeHtml } from '../escape.mjs';
import { parseComponent } from './schema.mjs';
import { budget, layout } from './bars.mjs';
import { compare, matrix, riskmap } from './grids.mjs';
import { layers, waves } from './graphs.mjs';
import { cards } from './cards.mjs';
import { stack, steps, timeline, tree } from './lists.mjs';
import { endpoint, examples, trace } from './tables.mjs';

const COMPONENTS = new Map(
  [tree, timeline, budget, layout, compare, matrix, riskmap, stack, layers, waves, cards, steps, endpoint, examples, trace].map((c) => [c.name, c]),
);

export const COMPONENT_LANGS = new Set(COMPONENTS.keys());

/**
 * @param {string} lang component name
 * @param {string} source YAML body
 * @param {number} firstLine plan line of the body's first line
 * @param {{ inline:(text:string, opts?:{inLink?:boolean})=>string, code:(text:string)=>string,
 *   fact:(word:string, ids?:string[], from?:"text"|"component")=>void,
 *   file:(path:string, status:string, label:string)=>string }} io
 * @returns {{ html:string, warnings:{text:string, line:number}[] }}
 */
export function renderComponent(lang, source, firstLine, io) {
  const component = COMPONENTS.get(lang);
  const { value, warnings } = parseComponent(lang, source, component.spec, firstLine);
  if (!value) {
    return {
      html: `<figure class="figure wide comp-failed"><div class="scroll"><pre class="diagram-src"><code>${escapeHtml(source)}</code></pre></div></figure>`,
      warnings: [...warnings, { text: `${lang}: not drawn; shown as source`, line: firstLine - 1, unrenderable: true }],
    };
  }
  const late = [];
  const h = { ...io, esc: escapeHtml, warn: (text, line) => late.push({ text: `${lang}: ${text}`, line }) };
  const body = component.render(value, h);
  const caption = value.caption ? `<figcaption>${io.inline(value.caption)}</figcaption>` : '';
  return {
    html: `<figure class="figure comp comp-${lang}${component.wide ? ' wide' : ''}">${caption}${body}</figure>`,
    warnings: [...warnings, ...late],
  };
}
