import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseEvidence } from '../lib/evidence.mjs';
import { render, tempRepo } from './helpers.mjs';

const mermaid = (body, info = '') => `# T\n\n\`\`\`mermaid${info}\n${body}\n\`\`\`\n`;
const RS = 'pub struct Rule {}\n\npub fn check() {}\n';
const GLOSSARY = '\n## Glossary\n\n- coordinator: the agent running the skill\n';

test('a focus node gets the focus class and a legend entry', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse]:::focus --> B[Store]'));
  assert.ok(html.includes('<li class="sem sem-focus"><span class="sem-swatch" aria-hidden="true"></span>focus</li>'));
});

test('a user node gets the user class and a legend entry', async () => {
  const { html } = await render(mermaid('flowchart LR\n  O([Owner]):::user --> B[Store]'));
  assert.ok(html.includes('<li class="sem sem-user"><span class="sem-swatch" aria-hidden="true"></span>user</li>'));
});

test('a sequence rect marked focus is drawn with the focus class', async () => {
  const { html } = await render(mermaid('sequenceDiagram\n  A->>B: ask\n  rect focus\n    B->>A: answer\n  end'));
  assert.match(html, /<rect class="pele-rect focus" [^>]*style="rx:var\(--_r\);"\/>/);
});

test('a sequence rect with a raw colour warns with the focus rect to use', async () => {
  const { warnings } = await render(mermaid('sequenceDiagram\n  A->>B: ask\n  rect rgb(255, 244, 200)\n    B->>A: answer\n  end'));
  assert.deepEqual(warnings, ['Mermaid rect sets a raw colour (rgb(255, 244, 200)), which ignores dark mode; write `rect focus` to mark the span the figure is about — plan.md:6']);
});

test('a classDef restyling a semantic class still applies its raw colour', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse]:::new --> B\n  classDef new fill:#fef3c7,stroke:#b45309'));
  assert.ok(html.split('</head>')[1].includes('#fef3c7'));
});

test('a classDef restyling a semantic class with a raw colour warns that it ignores dark mode', async () => {
  const { warnings } = await render(mermaid('flowchart LR\n  A[Parse]:::new --> B\n  classDef new fill:#fef3c7'));
  assert.deepEqual(warnings, ['Mermaid classDef new sets a raw colour (fill:#fef3c7), which ignores dark mode; use the semantic class `:::new` — plan.md:6']);
});

test('a semicolon in a sequence message renders instead of breaking the diagram', async () => {
  const { html, warnings } = await render(mermaid('sequenceDiagram\n  A->>B: skip rules; open the rest'));
  assert.deepEqual({ drawn: />skip rules; open the rest</.test(html), warnings }, { drawn: true, warnings: [] });
});

test('a node naming a glossary term links its entry with the definition as its title', async () => {
  const { html } = await render(`${mermaid('flowchart LR\n  C[Coordinator] --> D[Disk]')}${GLOSSARY}`);
  assert.match(html, /<a class="node-link node-term" href="#g-coordinator"><title>the agent running the skill<\/title><g class="pele-node[^"]*" data-id="C"/);
});

test('a node naming a repository symbol links its code', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(mermaid('flowchart LR\n  C["src/a.rs::check"] --> D[Disk]'), { roots: [root] });
  assert.match(html, /<a class="node-link node-code" href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/a\.rs#L3"><title>src\/a\.rs:L3<\/title><g class="pele-node/);
});

test('a node naming a module path resolves to the module file', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(mermaid('flowchart LR\n  C["a::check"] --> D[Disk]'), { roots: [root] });
  assert.match(html, /<a class="node-link node-code" href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/a\.rs#L3">/);
});

test('a node naming a cited symbol shows its evidence in the title', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE4 via:read `src/a.rs::check` — returns early\n', 'log.md') };
  const { html } = await render(mermaid('flowchart LR\n  C["a::check"] --> D[Disk]'), { roots: [root], evidence });
  assert.ok(html.includes('<title>src/a.rs:L3 · E4 — returns early</title>'));
});

test('a node naming neither a term nor a symbol is not a link', async () => {
  const { html } = await render(mermaid('flowchart LR\n  C[Parse] --> D[Disk]'));
  assert.equal(html.split('</head>')[1].includes('node-link'), false);
});

test('a sequence participant naming a term links like a node', async () => {
  const { html } = await render(`${mermaid('sequenceDiagram\n  participant Coordinator\n  Coordinator->>Coordinator: plans')}${GLOSSARY}`);
  assert.match(html, /<a class="node-link node-term" href="#g-coordinator"><title>the agent running the skill<\/title><g class="pele-node pele-actor/);
});

test('a module path in prose links its module file', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nThen `a::check` runs.\n', { roots: [root] });
  assert.ok(html.includes('<a class="path" href="https://github.com/acme/w/blob/main/src/a.rs#L3">'));
});

test('flowchart edges name the nodes they join', async () => {
  const { html } = await render(mermaid('flowchart LR\n  A[Parse] e1@--> B[Store]'));
  assert.match(html, /<g class="pele-edge" data-id="e1" data-from="A" data-to="B">/);
});

test('a journey keeps its one wide layout, which scrolls on a phone', async () => {
  const { html } = await render(mermaid('journey\n  title Owner\n  section Start\n    Owner runs the review command from a terminal: 5: Owner\n  section Review\n    Reviewers get a language-narrowed index of rules: 4: Owner'));
  assert.equal(html.split('</head>')[1].includes('is-narrow'), false);
});
