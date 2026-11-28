// Table-shaped contracts: an HTTP endpoint, worked examples (given / when / then), and a trace of operations in lanes.
import { caption, glyph, kindClass, nodeKind } from './kinds.mjs';
import { LINE, t } from './schema.mjs';

const scrollTable = (cls, inner) => `<div class="scroll table-wrap ${cls}-wrap"><table class="${cls}">${inner}</table></div>`;

// --- endpoint -------------------------------------------------------------------------------------------

/** Pure: a status code's colour kind — success passes, redirects are context, client errors risk, server errors fail. */
const statusKind = (status) => ({ 2: 'new', 3: 'context', 4: 'risk', 5: 'failure' })[Math.floor(status / 100)] ?? 'plain';

const body = () => t.map({ type: t.text({ optional: true }), example: t.text({ optional: true }) });

export const endpoint = {
  name: 'endpoint',
  wide: true,
  spec: t.map({
    caption: caption(),
    method: t.oneOf(['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']),
    path: t.text(),
    summary: t.text({ optional: true }),
    auth: t.exactlyOne({ scheme: t.text(), open: t.text() }),
    params: t.list(t.map({
      name: t.text(),
      in: t.oneOf(['path', 'query', 'header', 'body']),
      type: t.text(),
      required: t.bool({ optional: true, default: false }),
      note: t.text({ optional: true }),
    }), { optional: true, default: [], min: 0 }),
    request: { ...body(), optional: true },
    responses: t.list(t.map({
      status: t.num({ integer: true, min: 100, max: 599 }),
      meaning: t.text(),
      kind: t.oneOf(['new', 'changed', 'removed'], { optional: true }),
      headers: t.list(t.text(), { optional: true, default: [], min: 0 }),
      type: t.text({ optional: true }),
      example: t.text({ optional: true }),
    })),
  }),
  render({ method, path, summary, auth, params, request, responses }, h) {
    if (!responses.some((r) => r.status >= 400)) h.warn('no 4xx or 5xx response; document every failure the call can return', responses[0][LINE]);
    const authChip = auth.kind === 'open'
      ? `<span class="ep-auth is-open">${glyph('risk')}no auth: ${h.inline(auth.value)}</span>`
      : `<span class="ep-auth"><span class="glyph" aria-hidden="true">⚿</span>${h.inline(auth.value)}</span>`;
    const bodyOf = (b) => `${b.type ? `<span class="ep-ct">${h.esc(b.type)}</span>` : ''}`
      + `${b.example ? h.codeBlock(b.example, 'ep-ex', b.type) : ''}`;
    const paramRows = params.map((p) => `<tr><td><code>${h.esc(p.name)}</code></td><td><span class="loc">${p.in}</span></td>`
      + `<td><code>${h.esc(p.type)}</code></td><td>${p.required ? '<span class="req">required</span>' : ''}</td><td>${p.note ? h.inline(p.note) : ''}</td></tr>`).join('');
    const responseRows = responses.map((r) => `<tr${r.kind ? ` class="row-k-${r.kind}"` : ''}>`
      + `<td><span class="st ${kindClass(statusKind(r.status))}">${r.status}</span>${r.kind ? `<span class="badge badge-${r.kind}">${r.kind}</span>` : ''}</td>`
      + `<td>${h.inline(r.meaning)}${r.headers.length ? `<span class="ep-hd">${r.headers.map((x) => `<code>${h.esc(x)}</code>`).join('')}</span>` : ''}</td>`
      + `<td>${bodyOf(r)}</td></tr>`).join('');
    return `<div class="ep"><div class="ep-h"><span class="verb">${method}</span><code class="ep-p">${h.esc(path)}</code>${authChip}</div>`
      + `${summary ? `<p class="ep-s">${h.inline(summary)}</p>` : ''}`
      + `${params.length ? scrollTable('ep-t', `<thead><tr><th scope="col">param</th><th scope="col">in</th><th scope="col">type</th><th scope="col"><span class="sr-only">required</span></th><th scope="col">note</th></tr></thead><tbody>${paramRows}</tbody>`) : ''}`
      + `${request ? `<div class="ep-req"><span class="ep-lbl">request body</span>${bodyOf(request)}</div>` : ''}`
      + `${scrollTable('ep-t', `<thead><tr><th scope="col">status</th><th scope="col">meaning</th><th scope="col">body</th></tr></thead><tbody>${responseRows}</tbody>`)}</div>`;
  },
};

// --- examples -------------------------------------------------------------------------------------------

const EXAMPLE_KIND = { wire: 'plain', boundary: 'risk', error: 'failure' };

export const examples = {
  name: 'examples',
  wide: true,
  spec: t.map({
    caption: caption(),
    rows: t.list(t.map({
      given: t.text({ optional: true }),
      when: t.text(),
      then: t.text(),
      kind: t.oneOf(Object.keys(EXAMPLE_KIND), { optional: true, default: 'wire' }),
    })),
  }),
  render({ rows }, h) {
    const tr = (r) => {
      const kind = EXAMPLE_KIND[r.kind];
      return `<tr class="ex-${r.kind}${kind === 'plain' ? '' : ` row-k-${kind}`}">`
        + `<td>${r.given ? h.inline(r.given) : '<span class="ex-none">—</span>'}</td>`
        + `<td>${kind === 'plain' ? '' : glyph(kind)}${h.inline(r.when)}</td><td>${h.inline(r.then)}</td></tr>`;
    };
    return scrollTable('ex', `<thead><tr><th scope="col">Given</th><th scope="col">When</th><th scope="col">Then</th></tr></thead><tbody>${rows.map(tr).join('')}</tbody>`);
  },
};

// --- trace ----------------------------------------------------------------------------------------------

export const trace = {
  name: 'trace',
  wide: true,
  spec: t.map({
    caption: caption(),
    ops: t.list(t.map({ op: t.text(), key: t.text(), kind: nodeKind() })),
    legend: t.list(t.map({ symbol: t.text(), meaning: t.text() }), { optional: true, default: [], min: 0 }),
  }),
  render({ ops, legend }, h) {
    const lanes = [...new Set(ops.map((o) => o.key))];
    const cell = (o, lane) => (o.key === lane
      ? `<td class="tr-c on ${kindClass(o.kind)}">${o.kind === 'plain' ? '' : glyph(o.kind)}${h.inline(o.op)}</td>`
      : '<td class="tr-c"></td>');
    const rows = lanes.map((lane) => `<tr><th scope="row">${h.inline(lane)}</th>${ops.map((o) => cell(o, lane)).join('')}</tr>`).join('');
    const key = legend.length
      ? `<ul class="tr-leg">${legend.map((l) => `<li><b>${h.inline(l.symbol)}</b> ${h.inline(l.meaning)}</li>`).join('')}</ul>`
      : '';
    return `${scrollTable('trace', `<tbody>${rows}</tbody>`)}${key}`;
  },
};
