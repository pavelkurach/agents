import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assignWaves, budgetGeometry, compareKind } from '../lib/components/testing.mjs';
import { pushed, render, tempRepo } from './helpers.mjs';

// The fence opens on plan line 3, so the YAML body's first line is plan line 4.
const block = (lang, yaml) => `# T\n\n\`\`\`${lang}\n${yaml}\n\`\`\`\n`;

test('tree groups files under their directory with status, delta and note', async () => {
  const { html } = await render(block('tree', 'files:\n  - path: src/json.rs\n    status: added\n    delta: +80 -2\n    note: encodes results'));
  assert.ok(html.includes('<li class="tree-dir"><span class="tree-dirname">src/</span><ul><li class="tree-file k-new"><span class="glyph" aria-hidden="true">+</span><span class="tree-name"><span class="path path-new" title="src/json.rs">'));
});

test('tree warns on an unknown status with field path and line', async () => {
  const { warnings } = await render(block('tree', 'files:\n  - path: a.rs\n    status: addded'));
  assert.deepEqual(warnings, ['tree.files[0].status: "addded" is not one of added, modified, removed, context — plan.md:6', 'tree.files: needs at least 1 valid item — plan.md:5', 'tree: not drawn; shown as source — plan.md:3']);
});

const TIMELINE_GATES = 'steps:\n  - { title: Canary, kind: gate }\n  - { title: Drop column, kind: oneway }';

test('timeline marks a gate step', async () => {
  const { html } = await render(block('timeline', TIMELINE_GATES));
  assert.ok(html.includes('<li class="tl-step tl-gate">'));
});

test('timeline states a missing rollback past a one-way step', async () => {
  const { html } = await render(block('timeline', TIMELINE_GATES));
  assert.ok(html.includes('<p class="tl-rollback tl-norollback"><span class="tl-label">No rollback</span> past this step</p>'));
});

test('timeline warns on a non-boolean current', async () => {
  const { warnings } = await render(block('timeline', 'steps:\n  - title: Ship\n    current: yes please'));
  assert.deepEqual(warnings, ['timeline.steps[0].current: must be true or false — plan.md:6']);
});

const BUDGET_OVER = 'unit: ms\nlimit: 100\nparts:\n  - { label: parse, value: 60 }\n  - { label: upload, value: 60 }';

test('budget over its limit hatches the excess', async () => {
  const { html } = await render(block('budget', BUDGET_OVER));
  assert.ok(html.includes('<span class="bud-bar bud-over" style="--from:83.33%;--len:16.67%">'));
});

test('budget over its limit states the excess', async () => {
  const { html } = await render(block('budget', BUDGET_OVER));
  assert.ok(html.includes('20 ms over'));
});

test('budget warns on a non-numeric value', async () => {
  const { warnings } = await render(block('budget', 'unit: ms\nlimit: 100\nparts:\n  - { label: a, value: 10 }\n  - { label: b, value: lots }'));
  assert.deepEqual(warnings, ['budget.parts[1].value: must be a number — plan.md:8']);
});

test('budget geometry splits a part that crosses the limit', () => {
  const g = budgetGeometry([{ value: 60 }, { value: 60 }], 100);
  assert.deepEqual(g.rows.map((r) => [r.from, r.inside, r.over]), [[0, 0.5, 0], [0.5, 1 / 3, 1 / 6]]);
});

const LAYOUT_FIELDS = 'fields:\n  - { name: magic, size: 4 }\n  - { name: pad, size: 12, kind: reserved }';

test('layout draws fields to scale', async () => {
  const { html } = await render(block('layout', LAYOUT_FIELDS));
  assert.ok(html.includes('<span class="lay-seg k-context is-reserved" style="flex-grow:12"></span>'));
});

test('layout labels each field with its offset and size', async () => {
  const { html } = await render(block('layout', LAYOUT_FIELDS));
  assert.ok(html.includes('<span class="lay-pos">@4 · 12 bytes</span>'));
});

test('layout warns on a negative size', async () => {
  const { warnings } = await render(block('layout', 'fields:\n  - { name: a, size: 4 }\n  - { name: b, size: -1 }'));
  assert.deepEqual(warnings, ['layout.fields[1].size: must be ≥ 0 — plan.md:6']);
});

test('compare classifies a row with differing sides as changed', () => {
  assert.equal(compareKind({ before: 'a', after: 'b' }), 'changed');
});

test('compare classifies a row with equal sides as same', () => {
  assert.equal(compareKind({ before: 'a', after: 'a' }), 'same');
});

test('compare classifies a row with only a before side as removed', () => {
  assert.equal(compareKind({ before: 'a' }), 'removed');
});

test('compare classifies a row with only an after side as added', () => {
  assert.equal(compareKind({ after: 'b' }), 'added');
});

test('compare renders a changed row as removed-before and added-after cells', async () => {
  const { html } = await render(block('compare', 'rows:\n  - { label: stdout, before: text, after: json }'));
  assert.ok(html.includes('<tr class="cmp-changed"><th scope="row">stdout</th><td class="cmp-before"><span class="glyph" aria-hidden="true">−</span><span class="cmp-text">text</span></td><td class="cmp-after"><span class="glyph" aria-hidden="true">+</span><span class="cmp-text">json</span></td></tr>'));
});

test('compare warns on a row with neither side', async () => {
  const { warnings } = await render(block('compare', 'rows:\n  - { before: a, after: b }\n  - { label: empty }'));
  assert.deepEqual(warnings, ['compare: a row needs before, after, or both — plan.md:6']);
});

test('matrix tints a cell by its kind', async () => {
  const { html } = await render(block('matrix', 'columns: [text, json]\nrows:\n  - label: ok\n    cells: [lines, { text: object, kind: new }]'));
  assert.ok(html.includes('<td class="mx-cell k-new mx-new"><span class="glyph" aria-hidden="true">+</span>object</td>'));
});

test('matrix warns when a row has the wrong number of cells', async () => {
  const { warnings } = await render(block('matrix', 'columns: [a, b]\nrows:\n  - label: r\n    cells: [x]'));
  assert.deepEqual(warnings, ['matrix: row "r" has 1 cells for 2 columns — plan.md:6']);
});

test('riskmap places a risk in its likelihood × impact cell', async () => {
  const { html } = await render(block('riskmap', 'risks:\n  - { title: disk fills, likelihood: high, impact: high }'));
  assert.ok(html.includes('<div class="rm-cell heat-4 is-hot"><ul><li class="rm-chip">disk fills</li></ul></div>'));
});

const RISKMAP_TWO = 'risks:\n  - { title: disk fills, likelihood: high, impact: high }\n  - { title: slow start, likelihood: low, impact: low }';

test('riskmap grid shows the first risk title', async () => {
  const { html } = await render(block('riskmap', RISKMAP_TWO));
  assert.ok(html.includes('<li class="rm-chip">disk fills</li>'));
});

test('riskmap grid shows the second risk title', async () => {
  const { html } = await render(block('riskmap', RISKMAP_TWO));
  assert.ok(html.includes('<li class="rm-chip">slow start</li>'));
});

test('riskmap grid shows no generated ids', async () => {
  const { html } = await render(block('riskmap', RISKMAP_TWO));
  assert.equal(/\bR\d\b|rm-id/.test(html), false);
});

test('riskmap warns on an unknown level', async () => {
  const { warnings } = await render(block('riskmap', 'risks:\n  - { title: a, likelihood: low, impact: huge }\n  - { title: b, likelihood: low, impact: low }'));
  assert.deepEqual(warnings, ['riskmap.risks[0].impact: "huge" is not one of low, medium, high — plan.md:5']);
});

test('stack nests called frames with their marks', async () => {
  const { html } = await render(block('stack', 'frames:\n  - call: main\n    calls:\n      - { call: json::errors, mark: added }'));
  assert.ok(html.includes('<ul><li class="st-frame k-new"><div class="st-row"><span class="glyph" aria-hidden="true">+</span><span class="st-call"><code class="code">json::<wbr>errors</code></span><span class="badge badge-added">added</span>'));
});

test('stack warns on an unknown field', async () => {
  const { warnings } = await render(block('stack', 'frames:\n  - { call: main, colour: red }'));
  assert.deepEqual(warnings, ['stack.frames[0].colour: unknown field; expected call, mark, note, calls — plan.md:5']);
});

test('layers strikes an absent crossing', async () => {
  const { html } = await render(block('layers', 'tiers:\n  - { name: CLI, nodes: [main] }\n  - { name: Core, nodes: [check] }\ncrossings:\n  - { from: check, to: main, absent: true }'));
  assert.ok(html.includes('<li class="cx cx-absent"><span class="glyph" aria-hidden="true">×</span><span class="cx-end">check</span>'));
});

test('layers warns on a crossing to a node in no tier', async () => {
  const { warnings } = await render(block('layers', 'tiers:\n  - { name: CLI, nodes: [main] }\ncrossings:\n  - { from: main, to: ghost }'));
  assert.deepEqual(warnings, ['layers: crossing names "ghost", which is in no tier — plan.md:7']);
});

test('waves are computed from needs', () => {
  const { wave } = assignWaves([{ title: 'a', needs: [] }, { title: 'b', needs: ['a'] }, { title: 'c', needs: ['a', 'b'] }]);
  assert.deepEqual([...wave], [['a', 1], ['b', 2], ['c', 3]]);
});

const WAVES_TWO = 'items:\n  - { title: Add flag }\n  - { title: Route errors, needs: [Add flag] }';

test('waves render a column per wave', async () => {
  const { html } = await render(block('waves', WAVES_TWO));
  assert.ok(html.includes('<p class="wave-head">Wave 2</p>'));
});

test('waves link a need by the needed item title', async () => {
  const { html } = await render(block('waves', WAVES_TWO));
  assert.ok(html.includes('<span class="tl-label">needs</span> <a href="#wv-0">Add flag</a>'));
});

test('waves show no item id', async () => {
  const { html } = await render(block('waves', 'items:\n  - { title: Add flag }'));
  assert.equal(html.includes('wv-id'), false);
});

test('waves warn on a need that is not an item', async () => {
  const { warnings } = await render(block('waves', 'items:\n  - { title: a, needs: [zz] }'));
  assert.deepEqual(warnings, ['waves: "a" needs "zz", which is not an item — plan.md:5']);
});

test('waves warn on a dependency cycle', async () => {
  const { warnings } = await render(block('waves', 'items:\n  - { title: a, needs: [b] }\n  - { title: b, needs: [a] }'));
  assert.deepEqual(warnings, ['waves: "b" needs "a", which needs it back (cycle); edge ignored — plan.md:6']);
});

const CARDS_SHAPE = 'decisions:\n  - question: Shape?\n    chosen: object\n    reversibility: one-way\n    rejected:\n      - { option: array, reason: ambiguous }';

test('cards show a reversibility badge', async () => {
  const { html } = await render(block('cards', CARDS_SHAPE));
  assert.ok(html.includes('<span class="badge badge-one-way">one-way</span>'));
});

test('cards list a rejected option with its reason', async () => {
  const { html } = await render(block('cards', CARDS_SHAPE));
  assert.ok(html.includes('<li><span class="dec-option">array</span><span class="dec-reason">ambiguous</span></li>'));
});

test('cards warn on a missing required field', async () => {
  const { warnings } = await render(block('cards', 'decisions:\n  - { question: A?, chosen: x, reversibility: internal }\n  - { question: B?, reversibility: internal }'));
  assert.deepEqual(warnings, ['cards.decisions[1].chosen: is required — plan.md:6']);
});

test('steps put each verb on the arrow to the next step', async () => {
  const { html } = await render(block('steps', 'steps:\n  - { node: parse, verb: validates }\n  - run'));
  assert.ok(html.includes('<li class="step k-plain"><span class="step-node">parse</span><span class="step-edge"><span class="step-verb">validates</span></span></li><li class="step k-plain"><span class="step-node">run</span></li>'));
});

test('steps warn on a step with no verb before the next', async () => {
  const { warnings } = await render(block('steps', 'steps:\n  - parse\n  - run'));
  assert.deepEqual(warnings, ['steps: step "parse" has no verb for the arrow to the next step — plan.md:5']);
});

test('a component with invalid YAML shows its source', async () => {
  const { html } = await render(block('steps', 'steps: [a, b'));
  assert.ok(html.includes('<figure class="figure wide comp-failed">'));
});

test('a component with invalid YAML warns of the YAML error', async () => {
  const { warnings } = await render(block('steps', 'steps: [a, b'));
  assert.ok(warnings[0].startsWith('steps: YAML error: '));
});

test('a component caption is the figure caption', async () => {
  const { html } = await render(block('steps', 'caption: Parsing comes first\nsteps: [{ node: a, verb: then }, b]'));
  assert.ok(html.includes('<figure class="figure comp comp-steps"><figcaption>Parsing comes first</figcaption>'));
});

test('a stack frame naming a repository file links it', async () => {
  const root = tempRepo({ files: { 'src/a.rs': 'pub fn check() {}\n' } });
  const { html } = await render(block('stack', 'frames:\n  - call: src/a.rs::check'), { roots: [root] });
  assert.match(html, /<span class="st-call"><a class="path" href="file:\/\/[^"]*\/src\/a\.rs"/);
});

test('a wave need whose title holds a path links the card and leaves the path unlinked', async () => {
  const root = tempRepo({ files: { 'src/a.rs': '' } });
  const { html } = await render(block('waves', 'items:\n  - { title: "Ship `src/a.rs`" }\n  - { title: Then, needs: ["Ship `src/a.rs`"] }'), { roots: [root] });
  assert.ok(html.includes('<a href="#wv-0">Ship <code class="code">src/<wbr>a.rs</code></a>'));
});

test('a one-way node carries the one-way glyph', async () => {
  const { html } = await render(block('layers', 'tiers:\n  - { name: Data, nodes: [{ name: migration, kind: oneway }] }'));
  assert.ok(html.includes('<li class="ly-node k-oneway"><span class="glyph" aria-hidden="true">⇥</span>migration</li>'));
});

const ENDPOINT = 'method: PUT\npath: /blobs/{id}\nauth: { scheme: bearer token }\nresponses:\n  - { status: 201, meaning: stored }\n  - { status: 503, meaning: breaker open, kind: new, headers: [Retry-After] }';

test('endpoint heads the contract with its verb, path and auth scheme', async () => {
  const { html } = await render(block('endpoint', ENDPOINT));
  assert.ok(html.includes('<div class="ep-h"><span class="verb">PUT</span><code class="ep-p">/blobs/{id}</code><span class="ep-auth"><span class="glyph" aria-hidden="true">⚿</span>bearer token</span></div>'));
});

test('endpoint without auth states why it is open', async () => {
  const { html } = await render(block('endpoint', 'method: GET\npath: /health\nauth: { open: load balancer probe }\nresponses:\n  - { status: 200, meaning: up }\n  - { status: 503, meaning: down }'));
  assert.ok(html.includes('<span class="ep-auth is-open"><span class="glyph" aria-hidden="true">!</span>no auth: load balancer probe</span>'));
});

test('endpoint auth naming both a scheme and a reason to be open warns', async () => {
  const { warnings } = await render(block('endpoint', 'method: GET\npath: /a\nauth: { scheme: token, open: why }\nresponses:\n  - { status: 200, meaning: ok }\n  - { status: 404, meaning: none }'));
  assert.deepEqual(warnings, ['endpoint.auth: give exactly one of scheme, open — plan.md:6', 'endpoint: not drawn; shown as source — plan.md:3']);
});

test('endpoint colours a response status by its class and marks a new response', async () => {
  const { html } = await render(block('endpoint', ENDPOINT));
  assert.ok(html.includes('<tr class="row-k-new"><td><span class="st k-failure">503</span><span class="badge badge-new">new</span></td><td>breaker open<span class="ep-hd"><code>Retry-After</code></span></td>'));
});

test('endpoint with no 4xx or 5xx response warns that failures are undocumented', async () => {
  const { warnings } = await render(block('endpoint', 'method: GET\npath: /a\nauth: { scheme: token }\nresponses:\n  - { status: 200, meaning: ok }\n  - { status: 304, meaning: cached }'));
  assert.deepEqual(warnings, ['endpoint: no 4xx or 5xx response; document every failure the call can return — plan.md:8']);
});

test('endpoint lists its params with location and required mark', async () => {
  const { html } = await render(block('endpoint', `${ENDPOINT}\nparams:\n  - { name: id, in: path, type: uuid, required: true }`));
  assert.ok(html.includes('<tr><td><code>id</code></td><td><span class="loc">path</span></td><td><code>uuid</code></td><td><span class="req">required</span></td><td></td></tr>'));
});

test('examples tint an error row and mark it with the failure glyph', async () => {
  const { html } = await render(block('examples', 'rows:\n  - { given: breaker open, when: PUT /blobs, then: 503, kind: error }'));
  assert.ok(html.includes('<tr class="ex-error row-k-failure"><td>breaker open</td><td><span class="glyph" aria-hidden="true">×</span>PUT /blobs</td><td>503</td></tr>'));
});

test('examples show a dash for a row with no given', async () => {
  const { html } = await render(block('examples', 'rows:\n  - { when: PUT /blobs, then: "201" }'));
  assert.ok(html.includes('<tr class="ex-wire"><td><span class="ex-none">—</span></td><td>PUT /blobs</td><td>201</td></tr>'));
});

test('trace draws one lane per key in first-seen order with each op in its own column', async () => {
  const { html } = await render(block('trace', 'ops:\n  - { op: W1, key: page-4 }\n  - { op: R1, key: page-9 }\n  - { op: W2, key: page-4 }'));
  assert.ok(html.includes('<tbody><tr><th scope="row">page-4</th><td class="tr-c on k-plain">W1</td><td class="tr-c"></td><td class="tr-c on k-plain">W2</td></tr><tr><th scope="row">page-9</th><td class="tr-c"></td><td class="tr-c on k-plain">R1</td><td class="tr-c"></td></tr></tbody>'));
});

test('trace colours an op by its kind and marks it with the kind glyph', async () => {
  const { html } = await render(block('trace', 'ops:\n  - { op: W1, key: page-4, kind: failure }'));
  assert.ok(html.includes('<td class="tr-c on k-failure"><span class="glyph" aria-hidden="true">×</span>W1</td>'));
});

test('cards show who owns a decision as a pill', async () => {
  const { html } = await render(block('cards', 'decisions:\n  - { question: A?, chosen: x, reversibility: one-way, owner: user }'));
  assert.ok(html.includes('<p class="dec-owner"><span class="dec-label">Owner</span><span class="owner owner-user">user</span></p>'));
});

test('cards warn on an owner that is neither user nor planner', async () => {
  const { warnings } = await render(block('cards', 'decisions:\n  - { question: A?, chosen: x, reversibility: internal, owner: team }'));
  assert.deepEqual(warnings, ['cards.decisions[0].owner: "team" is not one of user, planner — plan.md:5']);
});

test('a tree whose root names the repository links its files inside it', async () => {
  const repo = tempRepo({ files: { 'src/a.rs': 'fn a() {}\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(block('tree', 'root: w\nfiles:\n  - path: src/a.rs\n    status: modified'), { roots: [repo] });
  assert.ok(html.includes('<a class="path" href="https://github.com/acme/w/blob/main/src/a.rs" title="w/src/a.rs">'));
});

test('a tree whose root names the repository does not report its files missing', async () => {
  const repo = tempRepo({ files: { 'src/a.rs': 'fn a() {}\n' }, remote: 'git@github.com:acme/w.git' });
  pushed(repo);
  const { warnings } = await render(block('tree', 'root: w\nfiles:\n  - path: src/a.rs\n    status: modified'), { roots: [repo] });
  assert.deepEqual(warnings, []);
});

const JSON_BODY = "method: PUT\npath: /blobs/{id}\nauth: { scheme: bearer token }\nresponses:\n  - { status: 201, meaning: stored }\n  - { status: 503, meaning: breaker open, type: application/json, example: '{ \"error\": \"BreakerOpen\", \"attempts\": 0 }' }";

test('an endpoint body example that is JSON is highlighted as JSON', async () => {
  const { html } = await render(block('endpoint', JSON_BODY));
  assert.match(html, /<pre class="ep-ex shiki[^"]*"[^>]*><code><span class="line">.*"error"/);
});

test('an endpoint body example that is not JSON stays plain text', async () => {
  const { html } = await render(block('endpoint', JSON_BODY.replace(`'{ "error": "BreakerOpen", "attempts": 0 }'`, 'rate limited').replace('application/json', 'text/plain')));
  assert.ok(html.includes('<pre class="ep-ex"><code>rate limited</code></pre>'));
});

test('an inline code span holding JSON is highlighted as JSON', async () => {
  const { html } = await render('# T\n\nReturns `{ "ok": true }`.\n');
  assert.match(html, /<code class="code code-json"><span style="[^"]*--shiki-light/);
});
