// Proportion components: latency/size budget (a waterfall against the limit) and wire/memory layout (to scale).
import { caption, glyph, kindClass } from './kinds.mjs';
import { t } from './schema.mjs';

const pct = (x) => `${(Math.round(x * 10000) / 100).toString()}%`;
const fmt = (n) => (Number.isInteger(n) ? String(n) : String(Math.round(n * 100) / 100));
const qty = (n, unit) => `${fmt(n)} ${unit}`;

// --- budget ---------------------------------------------------------------------------------------------

/** Pure: each part's start, in-limit length and over-limit length, as fractions of the drawn scale. */
export function budgetGeometry(parts, limit) {
  const total = parts.reduce((s, p) => s + p.value, 0);
  const scale = Math.max(total, limit) || 1;
  let start = 0;
  const rows = parts.map((p) => {
    const end = start + p.value;
    const inside = Math.max(0, Math.min(end, limit) - start);
    const row = { part: p, from: start / scale, inside: inside / scale, over: (p.value - inside) / scale, overFrom: Math.max(start, limit) / scale };
    start = end;
    return row;
  });
  return { rows, total, scale, limitAt: limit / scale };
}

export const budget = {
  name: 'budget',
  wide: true,
  spec: t.map({
    caption: caption(),
    unit: t.text(),
    limit: t.num({ min: 0 }),
    parts: t.list(t.map({
      label: t.text(),
      value: t.num({ min: 0 }),
      kind: t.oneOf(['plain', 'new', 'risk', 'changed'], { optional: true, default: 'plain' }),
      note: t.text({ optional: true }),
    })),
  }),
  render({ unit, limit, parts }, h) {
    const g = budgetGeometry(parts, limit);
    const over = g.total - limit;
    const rows = g.rows.map((r) => `<li class="bud-row ${kindClass(r.part.kind)}${r.over > 0 ? ' is-over' : ''}">`
      + `<span class="bud-label">${r.part.kind === 'plain' ? '' : glyph(r.part.kind)}${h.inline(r.part.label)}</span>`
      + `<span class="bud-value">${qty(r.part.value, h.esc(unit))}</span>`
      + `<span class="bud-track" aria-hidden="true">${r.inside > 0 ? `<span class="bud-bar" style="--from:${pct(r.from)};--len:${pct(r.inside)}"></span>` : ''}`
      + `${r.over > 0 ? `<span class="bud-bar bud-over" style="--from:${pct(r.overFrom)};--len:${pct(r.over)}"></span>` : ''}</span>`
      + `${r.part.note ? `<span class="bud-note">${h.inline(r.part.note)}</span>` : ''}</li>`);
    const status = over > 0
      ? `<span class="bud-status k-failure">${glyph('failure')}${qty(over, h.esc(unit))} over</span>`
      : `<span class="bud-status k-new">${qty(-over, h.esc(unit))} to spare</span>`;
    return `<div class="budget" style="--limit:${pct(g.limitAt)}">`
      + `<div class="bud-axis" aria-hidden="true"><span class="bud-limit-label${g.limitAt > 0.6 ? ' is-right' : ''}">limit ${qty(limit, h.esc(unit))}</span></div>`
      + `<ol class="bud-rows">${rows.join('')}</ol>`
      + `<p class="bud-total"><span>Total <strong>${qty(g.total, h.esc(unit))}</strong> of ${qty(limit, h.esc(unit))}</span>${status}</p></div>`;
  },
};

// --- layout ---------------------------------------------------------------------------------------------

export const layout = {
  name: 'layout',
  wide: true,
  spec: t.map({
    caption: caption(),
    unit: t.text({ optional: true, default: 'bytes' }),
    fields: t.list(t.map({
      name: t.text(),
      size: t.num({ min: 0 }),
      kind: t.oneOf(['plain', 'new', 'changed', 'removed', 'reserved'], { optional: true, default: 'plain' }),
      note: t.text({ optional: true }),
    })),
  }),
  render({ unit, fields }, h) {
    const total = fields.reduce((s, f) => s + f.size, 0);
    let offset = 0;
    const placed = fields.map((f) => {
      const at = offset;
      offset += f.size;
      return { f, at };
    });
    const cls = (f) => `${kindClass(f.kind)}${f.kind === 'reserved' ? ' is-reserved' : ''}`;
    const strip = placed.map(({ f }) => `<span class="lay-seg ${cls(f)}" style="flex-grow:${f.size}"></span>`).join('');
    const cells = placed.map(({ f, at }) => `<li class="lay-field ${cls(f)}" style="flex-grow:${f.size}">`
      + `<span class="lay-name">${f.kind === 'plain' ? '' : glyph(f.kind)}${h.inline(f.name)}</span>`
      + `<span class="lay-pos">@${fmt(at)} · ${qty(f.size, h.esc(unit))}</span>`
      + `${f.note ? `<span class="lay-note">${h.inline(f.note)}</span>` : ''}</li>`).join('');
    return `<div class="layout"><div class="lay-strip" aria-hidden="true">${strip}</div>`
      + `<ol class="lay-fields">${cells}</ol><p class="lay-total">${qty(total, h.esc(unit))} total · drawn to scale in the strip above</p></div>`;
  },
};
