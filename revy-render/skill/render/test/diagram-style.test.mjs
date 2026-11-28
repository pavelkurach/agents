// Diagram typography, code pills, outcome colours (authored `ok`, inferred ok / failure) and re-layout to the column.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render } from './helpers.mjs';

const mermaid = (body) => `# T\n\n\`\`\`mermaid\n${body}\n\`\`\`\n`;
const body = (html) => html.split('</head>')[1];
const TO_BE = [
  'flowchart LR',
  '  A[main::parse] -->|Check Json| B[cli::check]',
  '  A -->|--json on other command| X[usage error exit 2]',
  '  B --> C[checked]',
  '  C -->|Ok| D[ReviewDir::checked_files]',
  '  D --> E[json::success exit 0]',
  '  C -->|Err diagnostics| F[json::errors exit 1]',
  '  C -->|ShellError| G[json::failure exit 1]',
].join('\n');
const firstSvgWidth = (html) => Number(/<svg [^>]*class="pele[^"]*" viewBox="0 0 ([\d.]+)/.exec(body(html))[1]);

// --- code pills -------------------------------------------------------------------------------------------

test('a code label is drawn on a grey pill sized to its measured text', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[s3::put_object] --> B[Store]'));
  assert.ok(body(html).includes('<g class="code-pill"><rect x="-63" y="-7.98" width="126" height="17.5" rx="3.5"/></g><text class="pele-label" font-family="var(--_fm)"'));
});

test('a label mixing words and code puts only the code part on a pill', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[run `a::b` step]'));
  assert.ok(body(html).includes('<g class="code-pill"><rect x="-24.23" y="-7.98" width="42" height="17.5" rx="3.5"/></g><text class="pele-label" text-anchor="middle"'));
});

test('a label of plain words gets no pill', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[Store]'));
  assert.equal(body(html).includes('code-pill'), false);
});

test('code in an edge label is drawn on a pill too', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] -->|calls `put`| B[Store]'));
  assert.match(body(html), /<g class="pele-edge-label"[^>]*><rect [^>]*\/><g class="code-pill"><rect [^>]*height="15" rx="3"\/><\/g><text/);
});

// --- the ok class -----------------------------------------------------------------------------------------

test('an ok node gets the ok class and a legend entry', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[Saved]:::ok'));
  assert.ok(body(html).includes('<li class="sem sem-ok"><span class="sem-swatch" aria-hidden="true"></span>ok</li>'));
});

test('an ok node carries a check mark as its second signal', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[Saved]:::ok'));
  assert.match(body(html), /<g class="pele-node pele-shape-rect ok" data-id="B"[^]*?<g class="ok-mark" aria-hidden="true"><circle /);
});

test('an edge classed ok by its id gets the ok class', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] e1@--> B[Saved]\n  class e1 ok'));
  assert.match(body(html), /<g class="pele-edge ok" data-id="e1"/);
});

test('a labelled ok edge carries a check mark on its label', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] e1@-->|stored| B[Saved]\n  class e1 ok'));
  assert.match(body(html), /<g class="pele-edge-label ok" data-id="e1"[^>]*>(?:(?!<\/g>).)*?<g class="ok-mark" aria-hidden="true">/s);
});

// --- inferred outcome colours -----------------------------------------------------------------------------

test('an unclassed edge labelled with a failure word is inferred as failure', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|invalid input| B[Stop]'));
  assert.match(body(html), /<g class="pele-edge failure inferred" data-id="L_A_B_0"/);
});

test('an unclassed edge labelled ok is inferred as ok', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|ok| B[Use]'));
  assert.match(body(html), /<g class="pele-edge ok inferred" data-id="L_A_B_0"/);
});

test('an author class beats the class its label would infer', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] e1@-->|timeout| B[Retry]\n  class e1 risk'));
  assert.match(body(html), /<g class="pele-edge risk" data-id="e1"/);
});

test('an edge whose prose label names no outcome stays neutral', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|reads the file| B[Use]'));
  assert.match(body(html), /<g class="pele-edge" data-id="L_A_B_0"/);
});

test('a label naming both an ok and a failure word stays neutral', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|ok or error| B[Use]'));
  assert.match(body(html), /<g class="pele-edge" data-id="L_A_B_0"/);
});

test('a camel-case failure word in a label is inferred as failure', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|ShellError| B[Report]'));
  assert.match(body(html), /<g class="pele-edge failure inferred" data-id="L_A_B_0"/);
});

test('a flowchart cross arrow head is inferred as failure', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] --x B[Use]'));
  assert.match(body(html), /<g class="pele-edge failure inferred" data-id="L_A_B_0"/);
});

test('an unclassed terminal node naming a non-zero exit is inferred as failure', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> X[usage error exit 2]'));
  assert.match(body(html), /<g class="pele-node pele-shape-rect failure inferred" data-id="X"/);
});

test('an unclassed terminal node naming exit 0 is inferred as ok', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> E[print and exit 0]'));
  assert.match(body(html), /<g class="pele-node pele-shape-rect ok inferred" data-id="E"/);
});

test('a node with outgoing edges is not inferred from its label', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[error handler] --> B[Log]'));
  assert.match(body(html), /<g class="pele-node pele-shape-rect" data-id="A"/);
});

test('a sequence reply led by a 2xx status is inferred as ok', async () => {
  const { html } = await render(mermaid('sequenceDiagram\n  A->>B: put\n  B-->>A: 200 etag'));
  assert.match(body(html), /<g class="pele-edge pele-message ok inferred" data-id="i1"/);
});

test('a status-like number inside a sentence is not an outcome', async () => {
  const { html } = await render(mermaid('sequenceDiagram\n  A->>B: retry after 200 ms\n  B-->>A: done'));
  assert.match(body(html), /<g class="pele-edge pele-message" data-id="i0"/);
});

test('an inferred outcome gets its legend entry', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Load] -->|invalid input| B[Stop]'));
  assert.ok(body(html).includes('<li class="sem sem-failure"><span class="sem-swatch" aria-hidden="true"></span>failure</li>'));
});

// --- layout -----------------------------------------------------------------------------------------------

test('a flowchart too wide for the column is re-laid out to fit it', async () => {
  const { html } = await render(mermaid(TO_BE));
  assert.ok(firstSvgWidth(html) <= 1080, `wide layout is ${firstSvgWidth(html)} px`);
});

test('a re-laid-out flowchart keeps its node links and their titles', async () => {
  const { html } = await render(`${mermaid(TO_BE)}\n## Glossary\n\n- checked: the parsed review directory\n`);
  assert.match(body(html), /<a class="node-link node-term" href="#g-checked"><title>the parsed review directory<\/title><g class="pele-node/);
});

test('a flowchart that fits the column keeps its authored layout', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[Store]'));
  assert.match(body(html), /<g class="pele-node pele-shape-rect" data-id="B" transform="translate\(([\d.]+),30\.5\)"/);
});
