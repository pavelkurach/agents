// Shared vocabulary for components: semantic kinds, their glyph (the non-colour signal) and colour class.
import { escapeHtml } from '../escape.mjs';
import { t } from './schema.mjs';

/** kind -> glyph shown beside the colour. */
export const GLYPH = Object.freeze({
  new: '+', added: '+', changed: '~', modified: '~', removed: '−', failure: '×', risk: '!', oneway: '⇥',
  accepted: '✓', context: '·', differs: '≠', same: '=', reserved: '▨', gate: '◆', called: '›',
});

/** Kinds whose colour class differs from their word. */
const KIND_CLASS = Object.freeze({ added: 'new', modified: 'changed', differs: 'changed', same: 'context', reserved: 'context', called: 'plain' });
export const kindClass = (kind) => `k-${KIND_CLASS[kind] ?? kind}`;

export const glyph = (kind) => `<span class="glyph" aria-hidden="true">${escapeHtml(GLYPH[kind] ?? '')}</span>`;

/** Node kinds shared by steps, layers, waves. */
export const NODE_KINDS = ['plain', 'new', 'changed', 'removed', 'failure', 'risk', 'oneway', 'accepted', 'context'];
export const nodeKind = () => t.oneOf(NODE_KINDS, { optional: true, default: 'plain' });
export const caption = () => t.text({ optional: true });
