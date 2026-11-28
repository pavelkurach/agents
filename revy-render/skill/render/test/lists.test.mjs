import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render } from './helpers.mjs';

const body = (html) => html.split('</head>')[1];
const RUN = '`a()` (x, y), `b()` (`z`), `c()`';

test('a bullet with a comma run of code items shows the run as a list after its lead-in', async () => {
  const { html } = await render(`# T\n\n- modified ${RUN}\n`);
  assert.ok(html.includes('<span class="badge badge-modified">modified</span> <span class="comma-list" role="list"><span class="comma-item" role="listitem"><code class="code">a()</code> (x, y),</span><span class="comma-item" role="listitem"> <code class="code">b()</code> (<code class="code">z</code>),</span><span class="comma-item" role="listitem"> <code class="code">c()</code></span></span></li>'));
});

test('a comma run shown as a list keeps every character', async () => {
  const { html } = await render(`# T\n\n- modified ${RUN}\n`);
  const li = /<li class="status-item[^>]*>(.*?)<\/li>/.exec(html)[1];
  assert.equal(li.replace(/<[^>]+>/g, ''), 'modified a() (x, y), b() (z), c()');
});

test('a comma run shown as a list warns the author', async () => {
  const { warnings } = await render(`# T\n\n- modified ${RUN}\n`);
  assert.deepEqual(warnings, ['comma-joined list of 3 items rendered as a list — write one item per line — plan.md:3']);
});

test('a comma run in a paragraph becomes a list inside it', async () => {
  const { html } = await render(`# T\n\nTouches ${RUN}.\n`);
  assert.ok(html.includes('<p>Touches <span class="comma-list" role="list"><span class="comma-item" role="listitem"><code class="code">a()</code> (x, y),</span>'));
});

test('a comma run in a table cell becomes a list inside it', async () => {
  const { html } = await render(`# T\n\n| what | files |\n| --- | --- |\n| x | ${RUN} |\n`);
  assert.ok(html.includes('<td><span class="comma-list" role="list"><span class="comma-item" role="listitem"><code class="code">a()</code>'));
});

test('two comma-joined code items stay inline', async () => {
  const { html } = await render('# T\n\n- modified `a()`, `b()`\n');
  assert.equal(body(html).includes('class="comma-list"'), false);
});

test('a run whose last item opens with and is still a list', async () => {
  const { html } = await render('# T\n\nUses `a()`, `b()`, and `c()`.\n');
  assert.ok(html.includes('<span class="comma-item" role="listitem"> and <code class="code">c()</code>.</span></span></p>'));
});

test('plain prose commas stay inline', async () => {
  const { html } = await render('# T\n\nFast, small, and simple.\n');
  assert.ok(html.includes('<p>Fast, small, and simple.</p>'));
});

test('a run whose items carry words after their code stays inline', async () => {
  const { html } = await render('# T\n\nWe call `a()` first, `b()` next, `c()` last.\n');
  assert.equal(body(html).includes('class="comma-list"'), false);
});

test('a lead-in that names code itself still leads the run', async () => {
  const { html } = await render('# T\n\n- `config.toml` keys `a`, `b`, `c`\n');
  assert.ok(html.includes('<li><code class="code">config.toml</code> keys <span class="comma-list" role="list">'));
});
