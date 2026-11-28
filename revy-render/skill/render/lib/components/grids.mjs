// Grid-shaped components: before/after compare, matrix, likelihood × impact risk map.
import { caption, glyph, kindClass } from './kinds.mjs';
import { LINE, t } from './schema.mjs';

const scrollTable = (cls, inner) => `<div class="scroll table-wrap ${cls}-wrap"><table class="${cls}">${inner}</table></div>`;

// --- compare --------------------------------------------------------------------------------------------

/** Pure: a row's kind from which sides it has and whether they differ. */
export const compareKind = ({ before, after }) => {
  if (before && after) return before === after ? 'same' : 'changed';
  return before ? 'removed' : 'added';
};

export const compare = {
  name: 'compare',
  wide: true,
  spec: t.map({
    caption: caption(),
    before: t.text({ optional: true, default: 'Before' }),
    after: t.text({ optional: true, default: 'After' }),
    rows: t.list(t.map({ label: t.text({ optional: true }), before: t.text({ optional: true }), after: t.text({ optional: true }) })),
  }),
  render({ before, after, rows }, h) {
    rows.filter((r) => !r.before && !r.after).forEach((r) => h.warn('a row needs before, after, or both', r[LINE]));
    const kept = rows.filter((r) => r.before || r.after);
    const labelled = kept.some((r) => r.label);
    const none = '<span class="cmp-none">none</span>';
    const tr = (r) => {
      const kind = compareKind(r);
      const head = labelled ? `<th scope="row">${r.label ? h.inline(r.label) : ''}</th>` : '';
      if (kind === 'same') return `<tr class="cmp-same">${head}<td colspan="2"><span class="glyph" aria-hidden="true">=</span>${h.inline(r.before)} <span class="kword">unchanged</span></td></tr>`;
      const left = r.before ? `${glyph('removed')}<span class="cmp-text">${h.inline(r.before)}</span>` : none;
      const right = r.after ? `${glyph('added')}<span class="cmp-text">${h.inline(r.after)}</span>` : `${none} <span class="kword">removed</span>`;
      return `<tr class="cmp-${kind}">${head}<td class="cmp-before">${left}</td><td class="cmp-after">${right}</td></tr>`;
    };
    return scrollTable('compare', `<thead><tr>${labelled ? '<th scope="col"><span class="sr-only">Aspect</span></th>' : ''}`
      + `<th scope="col">${h.inline(before)}</th><th scope="col">${h.inline(after)}</th></tr></thead><tbody>${kept.map(tr).join('')}</tbody>`);
  },
};

// --- matrix ---------------------------------------------------------------------------------------------

const CELL_KINDS = ['plain', 'same', 'differs', 'new', 'removed', 'risk', 'failure', 'context'];

export const matrix = {
  name: 'matrix',
  wide: true,
  spec: t.map({
    caption: caption(),
    corner: t.text({ optional: true }),
    columns: t.list(t.text()),
    rows: t.list(t.map({
      label: t.text(),
      cells: t.list(t.textOr('text', { text: t.text(), kind: t.oneOf(CELL_KINDS, { optional: true, default: 'plain' }) })),
    })),
  }),
  render({ corner, columns, rows }, h) {
    rows.filter((r) => r.cells.length !== columns.length).forEach((r) =>
      h.warn(`row "${r.label}" has ${r.cells.length} cells for ${columns.length} columns`, r[LINE]));
    const cell = (c) => (c
      ? `<td class="mx-cell ${kindClass(c.kind)} mx-${c.kind}">${c.kind === 'plain' ? '' : glyph(c.kind)}${h.inline(c.text)}</td>`
      : '<td class="mx-cell mx-empty"></td>');
    const body = rows.map((r) => `<tr><th scope="row">${h.inline(r.label)}</th>${columns.map((_, i) => cell(r.cells[i])).join('')}</tr>`).join('');
    return scrollTable('matrix', `<thead><tr><th scope="col">${corner ? h.inline(corner) : ''}</th>${columns.map((c) => `<th scope="col">${h.inline(c)}</th>`).join('')}</tr></thead><tbody>${body}</tbody>`);
  },
};

// --- riskmap --------------------------------------------------------------------------------------------

const LEVELS = ['low', 'medium', 'high'];

export const riskmap = {
  name: 'riskmap',
  spec: t.map({
    caption: caption(),
    risks: t.list(t.map({
      title: t.text(),
      likelihood: t.oneOf(LEVELS),
      impact: t.oneOf(LEVELS),
      note: t.text({ optional: true }),
    })),
  }),
  render({ risks }, h) {
    const chip = (r) => `<li class="rm-chip">${h.inline(r.title)}</li>`;
    const cells = [...LEVELS].reverse().flatMap((like) => [
      `<div class="rm-rowhead">${like}</div>`,
      ...LEVELS.map((imp) => {
        const here = risks.filter((r) => r.likelihood === like && r.impact === imp);
        const heat = LEVELS.indexOf(like) + LEVELS.indexOf(imp);
        return `<div class="rm-cell heat-${heat}${heat === 4 ? ' is-hot' : ''}">`
          + `${here.length ? `<ul>${here.map(chip).join('')}</ul>` : ''}</div>`;
      }),
    ]);
    // The grid is the picture; the list below, keyed by title, carries the same facts for screen readers and the notes.
    const list = risks.map((r) => '<li>'
      + `<span class="rm-title">${h.inline(r.title)}</span><span class="rm-score">${r.likelihood} likelihood · ${r.impact} impact</span>`
      + `${r.note ? `<span class="rm-note">${h.inline(r.note)}</span>` : ''}</li>`).join('');
    return '<div class="riskmap" aria-hidden="true"><div class="rm-y">likelihood</div><div class="rm-grid">'
      + `<div class="rm-corner"></div>${LEVELS.map((l) => `<div class="rm-colhead">${l}</div>`).join('')}${cells.join('')}</div>`
      + '<div class="rm-x">impact</div></div>'
      + `<ul class="rm-list">${list}</ul>`;
  },
};
