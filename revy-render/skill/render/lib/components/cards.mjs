// Decision cards: the question, what was chosen, what was rejected and why, and how reversible it is.
import { caption } from './kinds.mjs';
import { t } from './schema.mjs';

const REVERSIBILITY = ['one-way', 'reversible', 'internal'];

export const cards = {
  name: 'cards',
  wide: true,
  spec: t.map({
    caption: caption(),
    decisions: t.list(t.map({
      question: t.text(),
      chosen: t.text(),
      reversibility: t.oneOf(REVERSIBILITY),
      why: t.text({ optional: true }),
      cost: t.text({ optional: true }),
      owner: t.oneOf(['user', 'planner'], { optional: true }),
      rejected: t.list(t.textOr('option', { option: t.text(), reason: t.text({ optional: true }) }), { optional: true, default: [], min: 0 }),
    })),
  }),
  render({ decisions }, h) {
    decisions.forEach((d) => h.fact(d.reversibility, undefined, 'component'));
    const card = (d) => `<li class="dec dec-${d.reversibility}"><div class="dec-head"><p class="dec-q">${h.inline(d.question)}</p>`
      + `<span class="badge badge-${d.reversibility}">${d.reversibility}</span></div>`
      + `<div class="dec-chosen"><span class="dec-label"><span class="glyph" aria-hidden="true">✓</span>Chosen</span><div class="dec-text">${h.inline(d.chosen)}</div></div>`
      + `${d.why ? `<p class="dec-why"><span class="dec-label">Why</span> ${h.inline(d.why)}</p>` : ''}`
      + `${d.cost ? `<p class="dec-cost"><span class="dec-label">Cost</span> ${h.inline(d.cost)}</p>` : ''}`
      + `${d.rejected.length ? `<div class="dec-rejected"><span class="dec-label"><span class="glyph" aria-hidden="true">×</span>Rejected</span><ul>${d.rejected
        .map((r) => `<li><span class="dec-option">${h.inline(r.option)}</span>${r.reason ? `<span class="dec-reason">${h.inline(r.reason)}</span>` : ''}</li>`)
        .join('')}</ul></div>` : ''}`
      + `${d.owner ? `<p class="dec-owner"><span class="dec-label">Owner</span><span class="owner owner-${d.owner}">${d.owner}</span></p>` : ''}</li>`;
    return `<ul class="decisions">${decisions.map(card).join('')}</ul>`;
  },
};
