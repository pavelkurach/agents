// Graph-shaped components laid out by CSS: tiers with crossings, and dependency waves.
import { statusOf } from '../markers.mjs';
import { caption, glyph, kindClass, nodeKind } from './kinds.mjs';
import { LINE, t } from './schema.mjs';

// --- layers ---------------------------------------------------------------------------------------------

export const layers = {
  name: 'layers',
  wide: true,
  spec: t.map({
    caption: caption(),
    tiers: t.list(t.map({
      name: t.text(),
      note: t.text({ optional: true }),
      nodes: t.list(t.textOr('name', { name: t.text(), kind: nodeKind() })),
    })),
    crossings: t.list(t.map({
      from: t.text(),
      to: t.text(),
      label: t.text({ optional: true }),
      absent: t.bool({ optional: true, default: false }),
      note: t.text({ optional: true }),
    }), { optional: true, default: [], min: 0 }),
  }),
  render({ tiers, crossings }, h) {
    const names = new Set(tiers.flatMap((tr) => tr.nodes.map((n) => n.name)));
    crossings.forEach((c) => [c.from, c.to].filter((n) => !names.has(n)).forEach((n) =>
      h.warn(`crossing names "${n}", which is in no tier`, c[LINE])));
    const node = (n) => `<li class="ly-node ${kindClass(n.kind)}">${n.kind === 'plain' ? '' : glyph(n.kind)}${h.inline(n.name)}</li>`;
    const bands = tiers.map((tr) => `<li class="ly-tier"><div class="ly-head"><span class="ly-name">${h.inline(tr.name)}</span>`
      + `${tr.note ? `<span class="ly-note">${h.inline(tr.note)}</span>` : ''}</div><ul class="ly-nodes">${tr.nodes.map(node).join('')}</ul></li>`).join('');
    const cross = crossings.map((c) => `<li class="cx${c.absent ? ' cx-absent' : ''}">`
      + `${c.absent ? '<span class="glyph" aria-hidden="true">×</span>' : ''}<span class="cx-end">${h.inline(c.from)}</span>`
      + `<span class="cx-arrow" aria-hidden="true">${c.absent ? '⇢' : '→'}</span><span class="cx-end">${h.inline(c.to)}</span>`
      + `${c.label ? `<span class="cx-label">${h.inline(c.label)}</span>` : ''}${c.absent ? '<span class="kword">does not exist</span>' : ''}`
      + `${c.note ? `<span class="cx-note">${h.inline(c.note)}</span>` : ''}</li>`).join('');
    return `<ol class="layers">${bands}</ol>${crossings.length ? `<p class="cx-title">Crossings</p><ul class="crossings">${cross}</ul>` : ''}`;
  },
};

// --- waves ----------------------------------------------------------------------------------------------

/** Pure: wave number (1-based) per item title; an unknown need or a cycle edge is reported and ignored. */
export function assignWaves(items) {
  const byTitle = new Map(items.map((it) => [it.title, it]));
  const wave = new Map();
  const problems = [];
  const visit = (it, path) => {
    if (wave.has(it.title)) return wave.get(it.title);
    const deps = it.needs.flatMap((n) => {
      if (!byTitle.has(n)) {
        problems.push({ item: it, text: `"${it.title}" needs "${n}", which is not an item` });
        return [];
      }
      if (path.includes(n)) {
        problems.push({ item: it, text: `"${it.title}" needs "${n}", which needs it back (cycle); edge ignored` });
        return [];
      }
      return [visit(byTitle.get(n), [...path, n])];
    });
    const w = 1 + Math.max(0, ...deps);
    wave.set(it.title, w);
    return w;
  };
  items.forEach((it) => visit(it, [it.title]));
  return { wave, problems };
}

export const waves = {
  name: 'waves',
  wide: true,
  spec: t.map({
    caption: caption(),
    items: t.list(t.map({
      title: t.text(),
      needs: t.list(t.text(), { optional: true, default: [], min: 0 }),
      status: t.text({ optional: true }),
      note: t.text({ optional: true }),
    })),
  }),
  render({ items }, h) {
    items.filter((it, i) => items.findIndex((o) => o.title === it.title) !== i).forEach((it) => h.warn(`duplicate title "${it.title}"`, it[LINE]));
    const { wave, problems } = assignWaves(items);
    problems.forEach((p) => h.warn(p.text, p.item[LINE]));
    const count = Math.max(...wave.values());
    const indexOf = new Map(items.map((it, i) => [it.title, i]));
    const anchor = (title) => `wv-${indexOf.get(title)}`;
    const card = (it) => {
      const status = it.status ? statusOf(it.status) : undefined;
      const needs = it.needs.filter((n) => indexOf.has(n));
      return `<li class="wv-card" id="${anchor(it.title)}"><p class="wv-top">`
        + `${status ? `<span class="${status.cls}">${h.esc(status.word)}</span>${status.rest ? h.inline(status.rest) : ''}` : it.status ? h.inline(it.status) : ''}</p>`
        + `<p class="wv-title">${h.inline(it.title)}</p>${it.note ? `<p class="wv-note">${h.inline(it.note)}</p>` : ''}`
        + `<p class="wv-needs">${needs.length ? `<span class="tl-label">needs</span> ${needs.map((n) => `<a href="#${anchor(n)}">${h.inline(n, { inLink: true })}</a>`).join(', ')}` : '<span class="tl-label">needs nothing</span>'}</p></li>`;
    };
    const cols = Array.from({ length: count }, (_, i) => {
      const inWave = items.filter((it) => wave.get(it.title) === i + 1);
      return `<li class="wave"><p class="wave-head">Wave ${i + 1}${inWave.length > 1 ? `<span class="wave-count">${inWave.length} in parallel</span>` : ''}</p><ul class="wave-items">${inWave.map(card).join('')}</ul></li>`;
    });
    return `<div class="scroll waves-wrap"><ol class="waves" style="--waves:${count}">${cols.join('')}</ol></div>`;
  },
};
