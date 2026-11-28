import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseEvidence } from '../lib/evidence.mjs';
import { render, tempRepo } from './helpers.mjs';

const RS = Array.from({ length: 245 }, (_, i) => `// ${i + 1}`).join('\n');
const SNIPPET = '```rust src/a.rs:240-242 the early return\nfn a() {\n    return;\n}\n```\n';

test('a snippet fenced with its location numbers its lines from the first', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}`);
  assert.match(html, /<span class="line" id="snip-3-L240" data-line="240"><span[^>]*>fn<\/span>/);
});

test('a snippet numbers every line of its body and no more', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}`);
  assert.deepEqual([...html.matchAll(/data-line="(\d+)"/g)].map((m) => m[1]), ['240', '241', '242']);
});

test('a snippet cites its location as a link to those lines', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(`# T\n\n${SNIPPET}`, { roots: [root] });
  assert.match(html, /<figcaption><a class="path cite" href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/a\.rs#L240-L242">[^]*?<code>src\/<wbr>a\.rs:240-242<\/code><\/a> the early return<\/figcaption>/);
});

test('notes keyed by line after a snippet link and name their lines', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}\n- L241 — returns before the loop\n`);
  assert.ok(html.includes('<ul class="snippet-notes" data-snippet="snip-3">\n<li data-lines="241"><a class="ln" href="#snip-3-L241">L241</a> returns before the loop</li>'));
});

test('a note may name ranges and several lines', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}\n- L240-L241, L242 — the whole body\n`);
  assert.ok(html.includes('<li data-lines="240-241,242"><a class="ln" href="#snip-3-L240">L240-L241, L242</a> the whole body</li>'));
});

test('notes written as lines of a paragraph are tied the same way', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}\nL240 — opens\nL242 — closes\n`);
  assert.ok(html.includes('<ul class="snippet-notes" data-snippet="snip-3">\n<li data-lines="240"><a class="ln" href="#snip-3-L240">L240</a> opens</li>\n<li data-lines="242"><a class="ln" href="#snip-3-L242">L242</a> closes</li>\n</ul>'));
});

test('a note naming a line outside the snippet warns', async () => {
  const { warnings } = await render(`# T\n\n${SNIPPET}\n- L250 — not shown\n`);
  assert.deepEqual(warnings, ['snippet note L250 names a line outside the snippet (240-242) — plan.md:9']);
});

test('a list after a snippet that is not line notes stays a plain list', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}\n- returns early\n`);
  assert.ok(html.includes('<ul>\n<li>returns early</li>\n</ul>'));
});

test('notes after a block without a location number it from line 1', async () => {
  const { html } = await render('# T\n\n```rust\nfn a() {}\n```\n\n- L1 — the function\n');
  assert.ok(html.includes('<span class="line" id="snip-3-L1" data-line="1">'));
});

test('a code block without location or notes has no line numbers', async () => {
  const { html } = await render('# T\n\n```rust src/a.rs — entry\nfn a() {}\n```\n');
  assert.equal(html.split('</head>')[1].includes('data-line'), false);
});

test('an evidence id after a snippet location becomes its chip', async () => {
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE2 via:read `src/a.rs` — returns\n', 'log.md') };
  const { html } = await render('# T\n\n```rust src/a.rs:1-1 E2\nfn a() {}\n```\n', { evidence });
  assert.match(html, /<\/code><\/span> <a class="ev" href="#ev-E2"/);
});

test('a note whose line is outside the snippet keeps its key as text, linking nowhere', async () => {
  const { html } = await render(`# T\n\n${SNIPPET}\n- L250 — not shown\n`);
  assert.ok(html.includes('<li data-lines="250"><span class="ln">L250</span> not shown</li>'));
});

const WALK = '```rust src/a.rs:240-245 walk\nfn a() {\n    let x = 1;\n    let y = 2;\n    if x > y {\n        return;\n    }\n```\n';

test('a range note wraps its lines in a focusable chunk described by the note', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L242 — set up\n`);
  assert.ok(html.includes('<span class="chunk" tabindex="0" role="group" aria-label="Lines 241–242" aria-describedby="snip-3-n1" data-note="snip-3-n1"><span class="line" id="snip-3-L241" data-line="241">'));
});

test('a chunk closes after its last line', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L242 — set up\n`);
  assert.match(html, /id="snip-3-L242" data-line="242">[^\n]*<\/span><\/span><\/span>\n<span class="line" id="snip-3-L243"/);
});

test('lines outside every noted range stay plain code', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L242 — set up\n`);
  assert.match(html, /<code><span class="line" id="snip-3-L240" data-line="240">/);
});

test('the note of a chunk carries the id the chunk points at', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L242 — set up\n`);
  assert.ok(html.includes('<li id="snip-3-n1" data-lines="241-242"><a class="ln" href="#snip-3-L241">L241-L242</a> set up</li>'));
});

test('a range note written as a paragraph line makes the same chunk', async () => {
  const { html } = await render(`# T\n\n${WALK}\nL241-L242 — set up\n`);
  assert.ok(html.includes('<li id="snip-3-n1" data-lines="241-242">'));
});

test('a chunk touching the chunk before it takes the other tint', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L242 — set up\n- L243-L245 — the guard\n`);
  assert.ok(html.includes('<span class="chunk alt" tabindex="0" role="group" aria-label="Lines 243–245" aria-describedby="snip-3-n2" data-note="snip-3-n2">'));
});

test('chunks with plain lines between them share a tint', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L240-L241 — opens\n- L243-L245 — the guard\n`);
  assert.ok(html.includes('<span class="chunk" tabindex="0" role="group" aria-label="Lines 243–245"'));
});

test('a single-line note makes no chunk', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241 — one line\n`);
  assert.equal(html.split('</head>')[1].includes('class="chunk'), false);
});

test('a range overlapping an earlier chunk warns', async () => {
  const { warnings } = await render(`# T\n\n${WALK}\n- L241-L243 — set up\n- L243-L245 — the guard\n`);
  assert.deepEqual(warnings, ['snippet note L243-L245 overlaps the chunk L241-L243; only the first is a chunk — plan.md:13']);
});

test('a range overlapping an earlier chunk stays a note but makes no chunk', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L241-L243 — set up\n- L243-L245 — the guard\n`);
  assert.equal(html.split('</head>')[1].match(/class="chunk/g).length, 1);
});

test('a range that runs backwards warns', async () => {
  const { warnings } = await render(`# T\n\n${WALK}\n- L243-L241 — set up\n`);
  assert.deepEqual(warnings, ['snippet note L243-L241 runs backwards; write L241-L243 — plan.md:12']);
});

test('a range that runs backwards makes no chunk', async () => {
  const { html } = await render(`# T\n\n${WALK}\n- L243-L241 — set up\n`);
  assert.equal(html.split('</head>')[1].includes('class="chunk'), false);
});

test('a range reaching past the snippet warns and makes no chunk', async () => {
  const { html, warnings } = await render(`# T\n\n${WALK}\n- L244-L250 — runs off\n`);
  assert.deepEqual([warnings, html.split('</head>')[1].includes('class="chunk')], [['snippet note L250 names a line outside the snippet (240-245) — plan.md:12'], false]);
});
