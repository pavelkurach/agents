import assert from 'node:assert/strict';
import { test } from 'node:test';
import { segmentProse } from '../lib/autocode.mjs';
import { headSha, pushed, render, tempRepo, textOf } from './helpers.mjs';

const RS = 'pub struct Format {}\n\npub fn check() {}\n';

test('unpushed HEAD links the default branch', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root] });
  assert.match(html, /href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/a\.rs"/);
});

test('HEAD contained in a remote branch links the commit sha', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root] });
  assert.ok(html.includes(`/blob/${headSha(root)}/src/a.rs`));
});

test('path::symbol links the definition line', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `src/a.rs::check`.\n', { roots: [root] });
  assert.match(html, /src\/a\.rs#L3"/);
});

test('missing symbol links the file and warns', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  const { warnings } = await render('# T\n\nSee `src/a.rs::nope`.\n', { roots: [root] });
  assert.deepEqual(warnings, ['symbol not found, linking the file only: src/a.rs::nope — plan.md:3']);
});

test('repo without a remote gets a file:// link', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root] });
  assert.match(html, /href="file:\/\/[^"]*\/src\/a\.rs"/);
});

test('alias:path resolves in the aliased repo', async () => {
  const primary = tempRepo({ files: { 'x.rs': '' } });
  const other = tempRepo({ files: { 'src/b.rs': RS }, remote: 'git@github.com:acme/o.git' });
  const { html } = await render('# T\n\nSee `r1:src/b.rs`.\n', { roots: [primary, other] });
  assert.match(html, /github\.com\/acme\/o\/blob\/main\/src\/b\.rs/);
});

const ADDED_PLAN = '# T\n\nChanges:\n- added `src/new.rs`\n\nThe file `src/new.rs` is new.\n';

function addedPlanRepo() {
  const root = tempRepo({ files: { 'x.rs': '' }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  return root;
}

test('a missing file listed as added gets the new-file chip', async () => {
  const { html } = await render(ADDED_PLAN, { roots: [addedPlanRepo()] });
  assert.ok(html.includes('chip chip-new'));
});

test('a missing file listed as added gets no link', async () => {
  const { html } = await render(ADDED_PLAN, { roots: [addedPlanRepo()] });
  assert.equal(/href="[^"]*src\/new\.rs"/.test(html), false);
});

test('a missing file listed as added does not warn', async () => {
  const { warnings } = await render(ADDED_PLAN, { roots: [addedPlanRepo()] });
  assert.deepEqual(warnings, []);
});

test('a missing file not listed as added is path-styled and unlinked', async () => {
  const root = tempRepo({ files: { 'x.rs': '' } });
  const { html } = await render('# T\n\nSee `src/gone.rs`.\n', { roots: [root] });
  assert.ok(html.includes('class="path path-nolink"'));
});

test('a missing file not listed as added warns', async () => {
  const root = tempRepo({ files: { 'x.rs': '' } });
  const { warnings } = await render('# T\n\nSee `src/gone.rs`.\n', { roots: [root] });
  assert.deepEqual(warnings, ['path not found in any --repo and not added in Changes: src/gone.rs — plan.md:3']);
});

test('a plain code span stays a code pill', async () => {
  const { html } = await render('# T\n\nUse `foo bar`.\n');
  assert.ok(html.includes('<code class="code">foo bar</code>'));
});

const TABLE = '# T\n\n| Kind | State |\n|---|---|\n| a | one-way |\n| b | new `x_y` |\n| c | breaking |\n';

test('a status word as a whole cell becomes its badge', async () => {
  const { html } = await render(TABLE);
  assert.ok(html.includes('<span class="badge badge-one-way">one-way</span>'));
});

test('a status word as the first word of a cell becomes a badge and keeps the rest', async () => {
  const { html } = await render(TABLE);
  assert.ok(html.includes('<span class="badge badge-new">new</span> <code class="code">x_y</code>'));
});

test('Risk: line becomes a risk callout with its text intact', async () => {
  const { html } = await render('# T\n\nRisk: the disk fills.\n');
  assert.ok(html.includes('<p class="callout callout-risk"><span class="marker-label">Risk:</span> the disk fills.</p>'));
});

test('Mitigation: bullet marks the list item', async () => {
  const { html } = await render('# T\n\n- Mitigation: retry twice.\n');
  assert.ok(html.includes('<li class="callout callout-mitigation">'));
});

const ZZ_FENCE = '# T\n\n```zzlang\nhello <b>\n```\n';

test('unknown fence language renders plain', async () => {
  const { html } = await render(ZZ_FENCE);
  assert.ok(html.includes('<pre class="plain"><code>hello &lt;b&gt;\n</code></pre>'));
});

test('unknown fence language warns', async () => {
  const { warnings } = await render(ZZ_FENCE);
  assert.deepEqual(warnings, ['unknown code language "zzlang"; shown plain — plan.md:3']);
});

test('fence without a language renders plain and warns', async () => {
  const { warnings } = await render('# T\n\n```\nx\n```\n');
  assert.deepEqual(warnings, ['code block has no language; shown plain — plan.md:3']);
});

const RUST_FENCE = '# T\n\n```rust\nfn main() {}\n```\n';

test('known language is highlighted with a light-theme variable', async () => {
  const { html } = await render(RUST_FENCE);
  assert.ok(html.includes('--shiki-light:'));
});

test('known language is highlighted with a dark-theme variable', async () => {
  const { html } = await render(RUST_FENCE);
  assert.ok(html.includes('--shiki-dark:'));
});

test('known language does not warn', async () => {
  const { warnings } = await render(RUST_FENCE);
  assert.deepEqual(warnings, []);
});

const RAW_SCRIPT = '# T\n\n<script>alert(1)</script>\n';

test('raw html outside a figure fence is not emitted as a tag', async () => {
  const { html } = await render(RAW_SCRIPT);
  assert.equal(html.includes('<script>alert'), false);
});

test('raw html outside a figure fence is shown escaped', async () => {
  const { html } = await render(RAW_SCRIPT);
  assert.ok(html.includes('&lt;script&gt;'));
});

const SECTIONS = '# T\n\n## Alpha\n\na\n\n## Beta\n\nb\n\n## Gamma\n\nc\n';

test('look order puts listed sections first and keeps the rest in place', async () => {
  const { html } = await render(SECTIONS, { look: { order: ['Gamma'] } });
  const ids = [...html.matchAll(/<section class="section" id="([a-z]+)"/g)].map((m) => m[1]);
  assert.deepEqual(ids, ['gamma', 'alpha', 'beta']);
});

test('look fold turns a section into a closed details element', async () => {
  const { html } = await render(SECTIONS, { look: { fold: ['Beta'] } });
  assert.ok(html.includes('<details class="section folded" id="beta"><summary><h2>Beta</h2>'));
});

test('look emphasis on a title gives the section the emph class', async () => {
  const { html } = await render(SECTIONS, { look: { emphasis: ['Alpha'] } });
  assert.ok(html.includes('<section class="section emph" id="alpha">'));
});

test('look watermark renders a ribbon', async () => {
  const { html } = await render(SECTIONS, { look: { watermark: 'DRAFT' } });
  assert.ok(html.includes('<div class="ribbon" aria-hidden="true">DRAFT</div>'));
});

test('unknown look titles warn', async () => {
  const { warnings } = await render(SECTIONS, { look: { order: ['Nope'], fold: ['Nada'], emphasis: ['zzz'] } });
  assert.deepEqual(warnings, [
    'look.order names unknown section "Nope" — look file',
    'look.fold names unknown section "Nada" — look file',
    'look.emphasis matches no section or text "zzz" — look file',
  ]);
});

const codes = (t) => segmentProse(t).filter((s) => s.kind === 'code').map((s) => s.text);

test('snake_case, file:line and a::b are auto-monospaced', () => {
  assert.deepEqual(codes('run_render in main.rs:12 calls a::b now'), ['run_render', 'main.rs:12', 'a::b']);
});

test('code-shaped text inside a URL is not auto-monospaced', () => {
  assert.deepEqual(codes('see https://example.com/my_file.rs and run_it'), ['run_it']);
});

test('auto-monospace does not wrap a prose exclamation', () => {
  assert.deepEqual(codes('it is fast! really'), []);
});

test('auto-monospace leaves text inside an existing link alone', async () => {
  const { html } = await render('# T\n\n[my_link text](http://x.test)\n');
  assert.ok(html.includes('>my_link text</a>'));
});

const FIXTURE = [
  '# Fixture plan',
  '',
  'Goal: ship the thing.',
  '',
  '## Scope',
  '',
  '- first item',
  '- second item with `inline code`',
  '',
  '| Name | State |',
  '|---|---|',
  '| alpha | existing |',
  '',
  'Risk: it may break.',
  '',
  '```rust',
  'let answer = 42;',
  '```',
].join('\n');

test('every text item of the plan appears in the output', async () => {
  const { html } = await render(FIXTURE);
  const text = textOf(html);
  const expected = ['Fixture plan', 'Goal: ship the thing.', 'Scope', 'first item', 'second item with inline code', 'Name', 'State', 'alpha', 'existing', 'Risk: it may break.', 'let answer = 42;'];
  assert.deepEqual(expected.filter((e) => !text.includes(e)), []);
});

test('output makes no external requests', async () => {
  const { html } = await render(FIXTURE);
  assert.deepEqual(html.match(/(?:src|href)="https?:\/\/[^"]*"|url\(https?:|@import/g) ?? [], []);
});

test('a highlighted code fence keeps its caption as a figcaption', async () => {
  const { html } = await render('# T\n\n```rust src/lib.rs — entry point\nfn main() {}\n```\n');
  assert.ok(html.includes('<figure class="figure codefig wide"><figcaption>src/lib.rs — entry point</figcaption><div class="scroll wide codeblock">'));
});

test('a plain code fence keeps its caption as a figcaption', async () => {
  const { html } = await render('# T\n\n```text output of one run\nok\n```\n');
  assert.ok(html.includes('<figure class="figure codefig wide"><figcaption>output of one run</figcaption><div class="scroll wide codeblock">'));
});

test('a backticked path inside a markdown link renders as code, not a second link', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const { html } = await render('# T\n\n[see `src/a.rs`](https://x.test)\n', { roots: [root] });
  assert.ok(html.includes('<a href="https://x.test">see <code class="code">src/<wbr>a.rs</code></a>'));
});

test('a path-shaped span with no --repo given is path-styled', async () => {
  const { html } = await render('# T\n\nSee `src/gone.rs`.\n');
  assert.ok(html.includes('class="path path-nolink"'));
});

test('a path-shaped span with no --repo given does not warn', async () => {
  const { warnings } = await render('# T\n\nSee `src/gone.rs`.\n');
  assert.deepEqual(warnings, []);
});

test('an unpushed HEAD without origin/HEAD warns that links guess the main branch', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { warnings } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root] });
  assert.deepEqual(warnings, [`HEAD is on no remote branch and origin/HEAD is unset, so links point at branch "main"; push HEAD or run \`git remote set-head origin -a\` — --repo ${root}`]);
});

const FOOTNOTE = '# T\n\nClaim.[^1]\n\n[^1]: the source of the claim\n';

test('a footnote keeps its note text', async () => {
  const { html } = await render(FOOTNOTE);
  assert.ok(textOf(html).includes('the source of the claim'));
});

test('a footnote links its reference', async () => {
  const { html } = await render(FOOTNOTE);
  assert.ok(html.includes('href="#fn1"'));
});

test('footnotes render after the last section, not inside it', async () => {
  const { html } = await render('# T\n\n## A\n\nClaim.[^1]\n\n## B\n\nb\n\n[^1]: note\n', { look: { order: ['B', 'A'] } });
  assert.match(html, /<\/section>\n<section class="footnotes"/);
});

test('a bullet starting with a file status word gets that badge and the kind of its row', async () => {
  const { html } = await render('# T\n\n- removed `src/old.rs`\n');
  assert.ok(html.includes('<li class="status-item row-k-removed"><span class="badge badge-removed">removed</span> '));
});

const REMOTE_IMAGE = '# T\n\n![the chart](https://x.test/c.png)\n';

test('a remote image is not embedded: it becomes a link to its source', async () => {
  const { html } = await render(REMOTE_IMAGE);
  assert.ok(html.includes('<a class="image-link" href="https://x.test/c.png">the chart</a>'));
});

test('a remote image warns that it was linked instead', async () => {
  const { warnings } = await render(REMOTE_IMAGE);
  assert.deepEqual(warnings, ['image not embedded, linked instead (the page stays self-contained): https://x.test/c.png; draw it as a figure or a component — plan.md:3']);
});
