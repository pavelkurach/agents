// The step-through order a flowchart or state diagram carries to the page, and the `%% steps:` pin.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render } from './helpers.mjs';

const mermaid = (body) => `# T\n\n\`\`\`mermaid\n${body}\n\`\`\`\n`;
const stepsOf = (html) => JSON.parse((/<figure [^>]*data-steps="([^"]*)"/.exec(html)?.[1] ?? 'null').replace(/&quot;/g, '"').replace(/&amp;/g, '&'));

test('a flowchart carries its walk from the start node', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] --> B[Store]'));
  assert.deepEqual(stepsOf(html), [{ node: 'A', path: 0 }, { node: 'B', edge: 'L_A_B_0', path: 0 }]);
});

test('an edge into a failure node is walked after the main path', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A --> F[rejected]:::failure\n  A --> B[Store]'));
  assert.deepEqual(stepsOf(html), [{ node: 'A', path: 0 }, { node: 'B', edge: 'L_A_B_0', path: 0 }, { node: 'F', edge: 'L_A_F_0', path: 1 }]);
});

test('an edge whose label names a failure is walked after the main path', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A -->|timeout| R[Retry]\n  A --> B[Store]'));
  assert.deepEqual(stepsOf(html), [{ node: 'A', path: 0 }, { node: 'B', edge: 'L_A_B_0', path: 0 }, { node: 'R', edge: 'L_A_R_0', path: 1 }]);
});

test('a state diagram is walked transition by transition', async () => {
  const { html } = await render(mermaid('stateDiagram-v2\n  [*] --> Pending\n  Pending --> Sent: go'));
  assert.deepEqual(stepsOf(html), [{ node: 'root_start', path: 0 }, { node: 'Pending', edge: 'edge0', path: 0 }, { node: 'Sent', edge: 'edge1', path: 0 }]);
});

test('a transition inside a composite state keeps the id Pele draws it with', async () => {
  const { html } = await render(mermaid('stateDiagram-v2\n  [*] --> A\n  note right of A : waits\n  state B {\n    [*] --> Dial\n  }\n  A --> B: go'));
  assert.deepEqual(stepsOf(html), [{ node: 'root_start', path: 0 }, { node: 'A', edge: 'edge0', path: 0 }, { node: 'B', edge: 'edge3', path: 0 }, { node: 'B_start', path: 1 }, { node: 'Dial', edge: 'edge2', path: 1 }]);
});

test('a state transition names the states it joins', async () => {
  const { html } = await render(mermaid('stateDiagram-v2\n  Pending --> Sent: go'));
  assert.match(html, /<g class="pele-edge pele-transition" data-id="edge0" data-from="Pending" data-to="Sent">/);
});

test('a steps comment pins the walk', async () => {
  const { html } = await render(mermaid('flowchart LR\n  %% steps: A, C, B\n  A --> B\n  A --> C\n  C --> B'));
  assert.deepEqual(stepsOf(html), [{ node: 'A', path: 0 }, { node: 'C', edge: 'L_A_C_0', path: 0 }, { node: 'B', edge: 'L_C_B_0', path: 0 }]);
});

test('a steps comment naming an unknown node warns with the nodes there are', async () => {
  const { warnings } = await render(mermaid('flowchart LR\n  %% steps: A, Z, B\n  A --> B'));
  assert.deepEqual(warnings, ['Mermaid %% steps names "Z", which is no node of this diagram (nodes: A, B); it is skipped — plan.md:5']);
});

test('a steps comment drops the unknown node from the walk', async () => {
  const { html } = await render(mermaid('flowchart LR\n  %% steps: A, Z, B\n  A --> B'));
  assert.deepEqual(stepsOf(html), [{ node: 'A', path: 0 }, { node: 'B', edge: 'L_A_B_0', path: 0 }]);
});

test('a steps comment on a sequence diagram warns that it walks messages in order', async () => {
  const { warnings } = await render(mermaid('sequenceDiagram\n  %% steps: A, B\n  A->>B: ask'));
  assert.deepEqual(warnings, ['Mermaid %% steps only orders flowcharts and state diagrams; a sequence diagram steps through its messages in order — plan.md:5']);
});

test('a sequence diagram carries no walk', async () => {
  const { html } = await render(mermaid('sequenceDiagram\n  A->>B: ask\n  B->>A: answer'));
  assert.equal(stepsOf(html), null);
});

test('a flowchart with one node carries no walk', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Alone]'));
  assert.equal(stepsOf(html), null);
});

test('a state diagram starts at its top-level start even when a composite state is declared first', async () => {
  const { html } = await render(mermaid('stateDiagram-v2\n  state P {\n    [*] --> Q\n  }\n  [*] --> P'));
  assert.deepEqual(stepsOf(html), [{ node: 'root_start', path: 0 }, { node: 'P', edge: 'edge1', path: 0 }, { node: 'P_start', path: 1 }, { node: 'Q', edge: 'edge0', path: 1 }]);
});
