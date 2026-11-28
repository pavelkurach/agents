// List-shaped components: file tree, rollout timeline, call stack, step chain.
import { escapeHtml } from '../escape.mjs';
import { caption, glyph, kindClass, nodeKind } from './kinds.mjs';
import { LINE, t } from './schema.mjs';

// --- tree -----------------------------------------------------------------------------------------------

const FILE_STATUS = ['added', 'modified', 'removed', 'context'];

const delta = (d) => {
  if (!d) return '';
  const m = /^\s*\+?(\d+)\s*[-−]\s*(\d+)\s*$/.exec(d);
  return m
    ? `<span class="tree-delta" aria-label="${m[1]} lines added, ${m[2]} removed"><span class="d-add">+${m[1]}</span><span class="d-del">−${m[2]}</span></span>`
    : `<span class="tree-delta">${escapeHtml(d)}</span>`;
};

const dirOf = (p) => (p.includes('/') ? p.slice(0, p.lastIndexOf('/') + 1) : '');
const baseOf = (p) => p.slice(p.lastIndexOf('/') + 1);

export const tree = {
  name: 'tree',
  spec: t.map({
    caption: caption(),
    root: t.text({ optional: true }),
    files: t.list(t.map({
      path: t.text(),
      status: t.oneOf(FILE_STATUS),
      note: t.text({ optional: true }),
      delta: t.text({ optional: true }),
    })),
  }),
  render({ root, files }, h) {
    const groups = [...new Set(files.map((f) => dirOf(f.path)))].map((dir) => [dir, files.filter((f) => dirOf(f.path) === dir)]);
    const counts = FILE_STATUS.map((s) => [s, files.filter((f) => f.status === s).length]).filter(([, n]) => n);
    const full = (p) => (root ? `${root.replace(/\/$/, '')}/${p}` : p);
    files.forEach((f) => h.fact(f.status, [full(f.path)], 'component'));
    const row = (f) => `<li class="tree-file ${kindClass(f.status)}">${glyph(f.status)}`
      + `<span class="tree-name">${h.file(full(f.path), f.status, baseOf(f.path), root ? f.path : undefined)}</span>`
      + `<span class="badge badge-${f.status}">${f.status}</span>${delta(f.delta)}`
      + `${f.note ? `<span class="tree-note">${h.inline(f.note)}</span>` : ''}</li>`;
    const body = groups.map(([dir, fs]) => (dir
      ? `<li class="tree-dir"><span class="tree-dirname">${h.esc(dir)}</span><ul>${fs.map(row).join('')}</ul></li>`
      : fs.map(row).join(''))).join('');
    return `<div class="tree-head">${root ? `<code class="tree-root">${h.esc(root)}/</code>` : ''}`
      + `<span class="tree-counts">${counts.map(([s, n]) => `<span class="${kindClass(s)}">${glyph(s)}${n} ${s}</span>`).join('')}</span></div>`
      + `<ul class="tree">${body}</ul>`;
  },
};

// --- timeline -------------------------------------------------------------------------------------------

export const timeline = {
  name: 'timeline',
  spec: t.map({
    caption: caption(),
    steps: t.list(t.map({
      title: t.text(),
      when: t.text({ optional: true }),
      kind: t.oneOf(['step', 'gate', 'oneway'], { optional: true, default: 'step' }),
      detail: t.text({ optional: true }),
      rollback: t.text({ optional: true }),
      current: t.bool({ optional: true, default: false }),
    })),
  }),
  render({ steps: list }, h) {
    const tag = { gate: '<span class="tag tag-gate">gate</span>', oneway: '<span class="badge badge-one-way">one-way</span>', step: '' };
    const items = list.map((s) => {
      const rollback = s.rollback
        ? `<p class="tl-rollback"><span class="tl-label">Rollback</span> ${h.inline(s.rollback)}</p>`
        : s.kind === 'oneway' ? '<p class="tl-rollback tl-norollback"><span class="tl-label">No rollback</span> past this step</p>' : '';
      return `<li class="tl-step tl-${s.kind}${s.current ? ' is-current' : ''}"><span class="tl-dot" aria-hidden="true"></span>`
        + '<div class="tl-body"><p class="tl-head">'
        + `${s.when ? `<span class="tl-when">${h.inline(s.when)}</span>` : ''}<span class="tl-title">${h.inline(s.title)}</span>${tag[s.kind]}`
        + `${s.current ? '<span class="tag tag-current">now</span>' : ''}</p>`
        + `${s.detail ? `<p class="tl-detail">${h.inline(s.detail)}</p>` : ''}${rollback}</div></li>`;
    });
    return `<ol class="timeline">${items.join('')}</ol>`;
  },
};

// --- stack ----------------------------------------------------------------------------------------------

const MARKS = ['called', 'added', 'removed', 'changed', 'context'];

const frameSpec = () => t.map({
  call: t.text(),
  mark: t.oneOf(MARKS, { optional: true, default: 'called' }),
  note: t.text({ optional: true }),
  calls: { optional: true, default: [], describe: 'list of frames', check: (node, at) => t.list(frameSpec(), { min: 0 }).check(node, at) },
});

export const stack = {
  name: 'stack',
  spec: t.map({ caption: caption(), frames: t.list(frameSpec()) }),
  render({ frames }, h) {
    const frame = (f) => `<li class="st-frame ${kindClass(f.mark)}"><div class="st-row">${glyph(f.mark)}`
      + `<span class="st-call">${h.code(f.call)}</span>${f.mark === 'called' ? '' : `<span class="badge badge-${f.mark}">${f.mark}</span>`}`
      + `${f.note ? `<span class="st-note">${h.inline(f.note)}</span>` : ''}</div>`
      + `${f.calls.length ? `<ul>${f.calls.map(frame).join('')}</ul>` : ''}</li>`;
    return `<ul class="stack">${frames.map(frame).join('')}</ul>`;
  },
};

// --- steps ----------------------------------------------------------------------------------------------

export const steps = {
  name: 'steps',
  spec: t.map({
    caption: caption(),
    steps: t.list(t.textOr('node', { node: t.text(), verb: t.text({ optional: true }), kind: nodeKind() }), { min: 2 }),
  }),
  render({ steps: list }, h) {
    list.slice(0, -1).filter((s) => !s.verb).forEach((s) => h.warn(`step "${s.node}" has no verb for the arrow to the next step`, s[LINE]));
    const last = list.at(-1);
    if (last.verb) h.warn(`the last step "${last.node}" has a verb but no step follows it`, last[LINE]);
    const items = list.map((s, i) => `<li class="step ${kindClass(s.kind)}"><span class="step-node">${s.kind === 'plain' ? '' : glyph(s.kind)}${h.inline(s.node)}</span>`
      + `${i < list.length - 1 ? `<span class="step-edge">${s.verb ? `<span class="step-verb">${h.inline(s.verb)}</span>` : ''}</span>` : ''}</li>`);
    return `<ol class="steps">${items.join('')}</ol>`;
  },
};
