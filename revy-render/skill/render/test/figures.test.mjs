import assert from 'node:assert/strict';
import { test } from 'node:test';
import { monoLabels } from '../lib/diagram.mjs';
import { render } from './helpers.mjs';

const mermaid = (body, info = '') => `# T\n\n\`\`\`mermaid${info}\n${body}\n\`\`\`\n`;

const CAPTIONED_MERMAID = mermaid('flowchart LR\n  A[Parse] --> B[Store]', ' Parsing feeds the store');

test('a mermaid fence renders to inline SVG labelled by its caption', async () => {
  const { html } = await render(CAPTIONED_MERMAID);
  assert.match(html, /<svg[^>]*class="pele[^"]*"[^>]*role="img" aria-label="Parsing feeds the store"/);
});

test('a captioned mermaid fence does not warn', async () => {
  const { warnings } = await render(CAPTIONED_MERMAID);
  assert.deepEqual(warnings, []);
});

test('a mermaid figure without a caption is labelled by its first node', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse input] --> B[Store]'));
  assert.match(html, /role="img" aria-label="Parse input"/);
});

const FAILURE_NODE = mermaid('flowchart LR\n  A[Parse] --> E[Reject]:::failure');

test('a semantic node class reaches the SVG', async () => {
  const { html } = await render(FAILURE_NODE);
  assert.match(html, /class="pele-node [^"]*\bfailure"/);
});

test('the page maps a semantic node class to its token', async () => {
  const { html } = await render(FAILURE_NODE);
  assert.ok(html.includes('.pele-node.failure { --k: var(--failure);'));
});

test('class on a declared edge id puts the semantic class on that edge', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A e1@-->|no| E\n  class e1 failure'));
  assert.match(html, /class="pele-edge failure" data-id="e1"/);
});

test('a sequence message with a cross head is drawn as a failure', async () => {
  const { html } = await render(mermaid('sequenceDiagram\n  C->>S: upload\n  S--xC: 503'));
  assert.match(html, /class="pele-edge pele-message failure" data-id="i1"/);
});

test('a figure using semantic classes gets a legend naming them', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[New]:::new --> B[Old]:::removed'));
  assert.ok(html.includes('<ul class="legend" aria-label="Legend"><li class="sem sem-new"><span class="sem-swatch" aria-hidden="true"></span>new</li><li class="sem sem-removed">'));
});

const RAW_CLASSDEF = mermaid('flowchart LR\n  A --> B\n  classDef bad fill:#f00');

test('a classDef with a raw colour still renders', async () => {
  const { html } = await render(RAW_CLASSDEF);
  assert.ok(html.includes('<svg'));
});

test('a classDef with a raw colour warns with the semantic class to use', async () => {
  const { warnings } = await render(RAW_CLASSDEF);
  assert.deepEqual(warnings, ['Mermaid classDef bad sets a raw colour (fill:#f00), which ignores dark mode; use the semantic class `:::failure` — plan.md:6']);
});

test('a linkStyle with a raw colour warns with the edge-class form', async () => {
  const { warnings } = await render(mermaid('flowchart LR\n  A --> B\n  linkStyle 0 stroke:green'));
  assert.deepEqual(warnings, ['Mermaid linkStyle 0 sets a raw colour (stroke:green), which ignores dark mode; give the edge an id (`A e1@--> B`) and write `class e1 new` — plan.md:6']);
});

const BROKEN_MERMAID = mermaid('flowchart LR\n  A --> (');

test('a mermaid parse error shows the source', async () => {
  const { html } = await render(BROKEN_MERMAID);
  assert.ok(html.includes('<pre class="diagram-src" data-lang="mermaid"><code>flowchart LR\n  A --&gt; (\n</code></pre>'));
});

test('a mermaid parse error warns with the offending line', async () => {
  const { warnings } = await render(BROKEN_MERMAID);
  assert.deepEqual(warnings, ['Mermaid syntax error in "A --> (": Expecting \'NODE_STRING\', got \'PS\'; shown as source — plan.md:5']);
});

test('a diagram language without an engine is shown as source and warns', async () => {
  const { warnings } = await render('# T\n\n```plantuml\nA -> B\n```\n');
  assert.deepEqual(warnings, ['no diagram engine for "plantuml"; shown as source (write it as mermaid) — plan.md:3']);
});

test('an html-figure is emitted verbatim inside a figure with its caption', async () => {
  const { html } = await render('# T\n\n```html-figure Two boxes\n<div id="k"></div>\n```\n');
  assert.ok(html.includes('<figure class="figure figure-free wide"><figcaption>Two boxes</figcaption><div id="k"></div>\n</figure>'));
});

test('an svg-figure keeps its viewBox width and a min-width that holds its smallest text at 12 px', async () => {
  const { html } = await render('# T\n\n```svg-figure\n<svg aria-label="k" viewBox="0 0 700 200"><text font-size="10">a</text></svg>\n```\n');
  assert.ok(html.includes('<svg style="width:700px;max-width:100%;min-width:840px" aria-label="k" viewBox="0 0 700 200">'));
});

test('a raw hex colour in a free-form figure warns', async () => {
  const { warnings } = await render('# T\n\n```svg-figure\n<svg aria-label="x" viewBox="0 0 10 10"><rect fill="#ff0000"/></svg>\n```\n');
  assert.deepEqual(warnings, ['svg-figure: raw colour #ff0000 ignores dark mode; use a token (var(--ink), var(--new) …) or a kit class (.fig-node.k-new) — plan.md:4']);
});

test('a reference to a kit marker is not mistaken for a raw colour', async () => {
  const { warnings } = await render('# T\n\n```svg-figure\n<svg aria-label="x" viewBox="0 0 9 9"><path marker-end="url(#fig-arrow)"/><use href="#abc"/></svg>\n```\n');
  assert.deepEqual(warnings, []);
});

test('a free-form svg without an aria-label warns', async () => {
  const { warnings } = await render('# T\n\n```html-figure\n<div><svg viewBox="0 0 4 4"></svg></div>\n```\n');
  assert.deepEqual(warnings, ['html-figure: <svg> has no aria-label; add one stating what the figure shows — plan.md:4']);
});

test('a line starting with a near-miss marker warns naming the exact markers', async () => {
  const { warnings } = await render('# T\n\nAccepted unknown: the shape.\n');
  assert.deepEqual(warnings, ['"Accepted unknown:" is not a marker, so it is not coloured; start the line with exactly one of "Risk:", "Accepted:", "Blind spot:", "Mitigation:" — plan.md:3']);
});

test('a bare filename that does not resolve is a plain code pill', async () => {
  const { html } = await render('# T\n\nWrites `state.json`.\n');
  assert.ok(html.includes('<code class="code">state.json</code>'));
});

test('a bare filename that does not resolve does not warn', async () => {
  const { warnings } = await render('# T\n\nWrites `state.json`.\n');
  assert.deepEqual(warnings, []);
});

test('internal is a status word with its own badge', async () => {
  const { html } = await render('# T\n\n| Decision | Reversibility |\n|---|---|\n| where | internal |\n');
  assert.ok(html.includes('<span class="badge badge-internal">internal</span>'));
});

test('a table row with a status cell gets that status kind for its stripe', async () => {
  const { html } = await render('# T\n\n| File | Status |\n|---|---|\n| a.rs | removed |\n');
  assert.ok(html.includes('<tr class="row-k-removed">'));
});

test('the header counts files, decisions and risk markers from the document', async () => {
  const { html } = await render('# T\n\n## Changes\n\n- added `src/a.rs`, `src/b.rs`\n- modified `src/c.rs`\n\n| Decision | Reversibility |\n|---|---|\n| x | one-way |\n\nRisk: r.\n');
  assert.ok(html.includes('<dl class="facts"><div><dt>Files</dt><dd><span class="fact k-new"><b>2</b> added</span><span class="fact k-changed"><b>1</b> modified</span></dd></div><div><dt>Decisions</dt><dd><span class="fact k-oneway"><b>1</b> one-way</span></dd></div><div><dt>Risks</dt><dd><span class="fact k-risk"><b>1</b> risk</span></dd></div></dl>'));
});

test('the page embeds its fonts', async () => {
  const { html } = await render('# T\n');
  assert.ok(html.includes('src: url(data:font/woff2;base64,'));
});

test('the page opens folded sections for print', async () => {
  const { html } = await render('# T\n');
  assert.ok(html.includes("addEventListener('beforeprint'"));
});

test('a file listed in both a bullet and a table is counted once', async () => {
  const { html } = await render('# T\n\n- added `src/a.rs`\n\n| Status | Path |\n|---|---|\n| added | `src/a.rs` |\n');
  assert.ok(html.includes('<dt>Files</dt><dd><span class="fact k-new"><b>1</b> added</span></dd>'));
});

test('a mermaid label that is code is drawn in the mono face', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[call s3::put_object] --> B[done]'));
  assert.ok(html.includes('<tspan font-family="var(--_fm)">s3::put_object</tspan>'));
});

test('the accepted semantic class reaches the legend', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Known gap]:::accepted --> B'));
  assert.ok(html.includes('<li class="sem sem-accepted">'));
});

test('mono labels wrap code-shaped text in node shapes and leave node ids alone', () => {
  assert.equal(monoLabels('flowchart LR\n  put_blob[call s3::put_object] -->|on with_retry| B:::new'), 'flowchart LR\n  put_blob[call <code>s3::put_object</code>] -->|on <code>with_retry</code>| B:::new');
});

test('mono labels wrap code in a sequence message and leave its participants alone', () => {
  assert.equal(monoLabels('sequenceDiagram\n  api_server->>S: `put_object` once'), 'sequenceDiagram\n  api_server->>S: <code>put_object</code> once');
});

test('a free-form figure with a script warns', async () => {
  const { warnings } = await render('# T\n\n```html-figure\n<div></div><script>go()</script>\n```\n');
  assert.deepEqual(warnings, ['html-figure: <script> runs in the reader\'s browser; draw the figure without one — plan.md:4']);
});

test('a free-form figure loading a remote resource warns', async () => {
  const { warnings } = await render('# T\n\n```html-figure\n<img alt="x" src="https://x.test/a.png">\n```\n');
  assert.deepEqual(warnings, ['html-figure: src="https://x.test/a.png" loads from the network; the page must work offline — plan.md:4']);
});

test('a free-form figure styling with a remote url() warns', async () => {
  const { warnings } = await render('# T\n\n```html-figure\n<div style="background: url(http://x.test/b.png)"></div>\n```\n');
  assert.deepEqual(warnings, ['html-figure: url(http://x.test/b.png) loads from the network; the page must work offline — plan.md:4']);
});
