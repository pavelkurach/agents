// The one colour vocabulary: every coloured thing on the page (badge, diagram class, component cell, legend)
// is one of these kinds, and each kind pairs its colour with a second signal (glyph, stroke style, strike or word).
import { escapeHtml } from './escape.mjs';

/** Mermaid semantic classes, in legend order; `word` names the kind in the legend. */
export const SEMANTIC = Object.freeze([
  { cls: 'new', word: 'new' },
  { cls: 'changed', word: 'changed' },
  { cls: 'removed', word: 'removed' },
  { cls: 'ok', word: 'ok' },
  { cls: 'failure', word: 'failure' },
  { cls: 'risk', word: 'risk' },
  { cls: 'oneway', word: 'one-way' },
  { cls: 'accepted', word: 'accepted' },
  { cls: 'context', word: 'context' },
  { cls: 'focus', word: 'focus' },
  { cls: 'user', word: 'user' },
]);

/** A strip naming each semantic kind a figure uses, so colour is never the only signal. Empty when none. */
export function legendHtml(classes) {
  const kinds = SEMANTIC.filter((k) => classes.includes(k.cls));
  if (!kinds.length) return '';
  return `<ul class="legend" aria-label="Legend">${kinds
    .map((k) => `<li class="sem sem-${k.cls}"><span class="sem-swatch" aria-hidden="true"></span>${escapeHtml(k.word)}</li>`)
    .join('')}</ul>`;
}
