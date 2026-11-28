import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render, tempRepo } from './helpers.mjs';

const GLOSSARY = '\n\n## Glossary\n\n- probe: the one call let through\n- rule: one rule file\n- rule floor: rules every reviewer answers\n- `state.json`: review state\n';
const term = (href, title, text) => `<a class="term" href="${href}" title="${title}">${text}</a>`;

test('a glossary term in prose links its entry with the definition as its tip', async () => {
  const { html } = await render(`# T\n\nOne probe goes out.${GLOSSARY}`);
  assert.ok(html.includes(`<p>One ${term('#g-probe', 'the one call let through', 'probe')} goes out.</p>`));
});

test('a term matches regardless of case and keeps its own case', async () => {
  const { html } = await render(`# T\n\nProbe first.${GLOSSARY}`);
  assert.ok(html.includes(`<p>${term('#g-probe', 'the one call let through', 'Probe')} first.</p>`));
});

test('a plural of a term links the term', async () => {
  const { html } = await render(`# T\n\nTwo probes.${GLOSSARY}`);
  assert.ok(html.includes(`<p>Two ${term('#g-probe', 'the one call let through', 'probes')}.</p>`));
});

test('a word that only starts with a term is not linked', async () => {
  const { html } = await render(`# T\n\nProbing never stops.${GLOSSARY}`);
  assert.ok(html.includes('<p>Probing never stops.</p>'));
});

test('the longest term wins over a shorter one inside it', async () => {
  const { html } = await render(`# T\n\nThe rule floor holds.${GLOSSARY}`);
  assert.ok(html.includes(`<p>The ${term('#g-rule-floor', 'rules every reviewer answers', 'rule floor')} holds.</p>`));
});

test('a term in a heading is not linked', async () => {
  const { html } = await render(`# T\n\n## The probe\n\nx${GLOSSARY}`);
  assert.ok(html.includes('<h2>The probe</h2>'));
});

test('a term inside a code span is not linked', async () => {
  const { html } = await render(`# T\n\nCall \`send_probe()\`.${GLOSSARY}`);
  assert.ok(html.includes('<p>Call <code class="code">send_probe()</code>.</p>'));
});

test('a code span that is exactly a term links it', async () => {
  const { html } = await render(`# T\n\nWrites \`state.json\`.${GLOSSARY}`);
  assert.ok(html.includes(`<p>Writes ${term('#g-state-json', 'review state', '<code class="code">state.json</code>')}.</p>`));
});

test('a term inside a link is not linked again', async () => {
  const { html } = await render(`# T\n\nSee [the probe](#glossary).${GLOSSARY}`);
  assert.ok(html.includes('<a href="#glossary">the probe</a>'));
});

test('a term in a table cell is linked', async () => {
  const { html } = await render(`# T\n\n| a |\n|---|\n| a probe |${GLOSSARY}`);
  assert.ok(html.includes(`<td>a ${term('#g-probe', 'the one call let through', 'probe')}</td>`));
});

test('a definition that links a section sends a click on the term there', async () => {
  const { html } = await render('# T\n\nThe owner waits.\n\n## Glossary\n\n- owner: who runs it, see [Journey](#journey)\n\n## Journey\n\nx\n');
  assert.ok(html.includes(term('#journey', 'who runs it, see Journey', 'owner')));
});

test('a linked glossary entry carries the id its terms point at', async () => {
  const { html } = await render(`# T\n\nOne probe.${GLOSSARY}`);
  assert.ok(html.includes('<div id="g-probe"><dt>probe</dt>'));
});

test('a glossary entry nothing links to has no id', async () => {
  const { html } = await render(`# T\n\nOne probe.${GLOSSARY}`);
  assert.ok(html.includes('<div><dt>rule floor</dt>'));
});

test('a glossary under a label inside another section links its terms', async () => {
  const { html } = await render('# T\n\n## Scope\n\nThe coordinator reads it.\n\n## Orientation\n\nGlossary:\n- coordinator: the agent running the skill\n');
  assert.ok(html.includes(term('#g-coordinator', 'the agent running the skill', 'coordinator')));
});

test('a term naming several variants links each of them', async () => {
  const { html } = await render('# T\n\nUndecided, then Skipped.\n\n## Glossary\n\n- Undecided, Skipped: a specialist state\n');
  assert.ok(html.includes(`<p>${term('#g-undecided-skipped', 'a specialist state', 'Undecided')}, then ${term('#g-undecided-skipped', 'a specialist state', 'Skipped')}.</p>`));
});

test('a term in a component field is linked', async () => {
  const { html } = await render(`# T\n\n\`\`\`steps\nsteps: [{ node: send a probe, verb: then }, done]\n\`\`\`${GLOSSARY}`);
  assert.ok(html.includes(term('#g-probe', 'the one call let through', 'probe')));
});

test('a code span that is a repository path stays a path link, not a term', async () => {
  const root = tempRepo({ files: { 'src/state.json': '{}' } });
  const { html } = await render('# T\n\nReads `src/state.json`.\n\n## Glossary\n\n- src/state.json: the state\n', { roots: [root] });
  assert.equal(html.split('</head>')[1].includes('class="term"'), false);
});

test('a term in its own definition is not linked', async () => {
  const { html } = await render(`# T\n\nx${GLOSSARY}`);
  assert.ok(html.includes('<dt>rule</dt><dd>one rule file</dd>'));
});

test('a term joined to another word by a hyphen is part of that word, not linked', async () => {
  const { html } = await render(`# T\n\nThe rule-index and probe-based checks.${GLOSSARY}`);
  assert.ok(html.includes('<p>The rule-index and probe-based checks.</p>'));
});
