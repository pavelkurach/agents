import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseEvidence } from '../lib/evidence.mjs';
import { pushed, render, tempRepo } from './helpers.mjs';

const LOG = '# Log\n\n## Evidence\nE1 via:read `src/a.rs::check` — returns early on an empty list\nE2 via:read+via:grep 52 call sites | backs: the shape decision\n\n## Decisions\nE3 via:read not evidence\n';
const ledger = (text = LOG) => ({ name: 'log.md', ...parseEvidence(text, 'log.md') });

test('an evidence line splits into markers, citation and what it showed', () => {
  const { entries } = parseEvidence(LOG, 'log.md');
  assert.deepEqual(entries.get('E1'), { id: 'E1', line: 4, markers: ['read'], citation: '`src/a.rs::check`', what: 'returns early on an empty list', backs: undefined });
});

test('an evidence line without a dash has no citation and keeps its backs statement apart', () => {
  const { entries } = parseEvidence(LOG, 'log.md');
  assert.deepEqual(entries.get('E2'), { id: 'E2', line: 5, markers: ['read', 'grep'], citation: '', what: '52 call sites', backs: 'the shape decision' });
});

test('a bulleted evidence line with dot separators splits the same way', () => {
  const { entries } = parseEvidence('## Evidence\n- E7 · via:read · `cli/src/cli.rs` · L308-L331 · local only — every set\n', 'log.md');
  assert.deepEqual(entries.get('E7'), { id: 'E7', line: 2, markers: ['read'], citation: '`cli/src/cli.rs` · L308-L331 · local only', what: 'every set', backs: undefined });
});

test('evidence lines end at the next heading of the same level', () => {
  const { entries } = parseEvidence(LOG, 'log.md');
  assert.deepEqual([...entries.keys()], ['E1', 'E2']);
});

test('a log without an Evidence heading is rejected naming the file', () => {
  assert.throws(() => parseEvidence('# Log\n\nE1 via:read x — y\n', 'log.md'), { message: '--evidence log.md has no "Evidence" heading; its entries go under one, e.g. "## Evidence"' });
});

test('an evidence id listed twice warns and keeps the first', () => {
  const { warnings } = parseEvidence('## Evidence\nE1 via:read a — b\nE1 via:read c — d\n', 'log.md');
  assert.deepEqual(warnings, ['evidence E1 is listed twice; keeping the first — log.md:3']);
});

test('an evidence id in prose becomes a chip linking its appendix entry', async () => {
  const { html } = await render('# T\n\nIt returns early (E1).\n', { evidence: ledger() });
  assert.ok(html.includes('(<a class="ev" href="#ev-E1" title="E1 · src/a.rs::check — returns early on an empty list">E1</a>)'));
});

test('an evidence id in a table cell becomes a chip', async () => {
  const { html } = await render('# T\n\n| a | b |\n|---|---|\n| x | E2 |\n', { evidence: ledger() });
  assert.ok(html.includes('<td><a class="ev" href="#ev-E2" title="E2 — 52 call sites">E2</a></td>'));
});

test('without --evidence an id stays plain text', async () => {
  const { html } = await render('# T\n\nIt returns early (E1).\n');
  assert.ok(html.includes('<p>It returns early (E1).</p>'));
});

test('an id inside a code span is not a chip', async () => {
  const { html } = await render('# T\n\nThe `E1` constant.\n', { evidence: ledger() });
  assert.equal(html.split('</head>')[1].includes('class="ev"'), false);
});

test('an id with no ledger entry warns once and stays plain', async () => {
  const { html, warnings } = await render('# T\n\nSee E9 and E9.\n', { evidence: ledger() });
  assert.deepEqual({ plain: html.includes('<p>See E9 and E9.</p>'), warnings }, { plain: true, warnings: ['evidence E9 has no entry in log.md; shown as plain text — plan.md:3'] });
});

test('the evidence appendix follows the last section, folded', async () => {
  const { html } = await render('# T\n\n## One\n\nx\n\n## Two\n\ny\n', { evidence: ledger() });
  assert.match(html, /<section class="section" id="two">[^]*?<\/section>\n<details class="section folded evidence" id="evidence"><summary><h2>Evidence<\/h2><span class="digest">2 entries from log\.md<\/span><\/summary>/);
});

test('each appendix entry is a link target showing its markers', async () => {
  const { html } = await render('# T\n', { evidence: ledger() });
  assert.ok(html.includes('<li id="ev-E2"><span class="ev-id">E2</span><span class="ev-via">read + grep</span><span class="ev-what">52 call sites</span><span class="ev-backs"><span class="ev-label">backs</span> the shape decision</span></li>'));
});

test('an appendix citation links its path into the repo', async () => {
  const root = tempRepo({ files: { 'src/a.rs': 'pub fn check() {}\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n', { roots: [root], evidence: ledger() });
  assert.match(html, /<span class="ev-cite"><a class="path" href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/a\.rs#L1">/);
});

test('the side navigation lists the evidence appendix', async () => {
  const { html } = await render('# T\n\n## One\n\nx\n', { evidence: ledger() });
  assert.ok(html.includes('<li><a href="#one">One</a></li><li><a href="#evidence">Evidence</a></li>'));
});

test('a backticked marker is read as a marker', () => {
  const { entries } = parseEvidence('## Evidence\n- E1 `via:read` docs/spec.md — no design\n', 'log.md');
  assert.deepEqual(entries.get('E1').markers, ['read']);
});

test('a comma run of code in a ledger line is not reported', async () => {
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE1 via:read `src/a.rs` — touches `a()`, `b()`, `c()`\n', 'log.md') };
  const { warnings } = await render('# T\n\nSee E1.\n', { evidence });
  assert.deepEqual(warnings, []);
});

test('warnings about the ledger itself follow the document warnings under a count', async () => {
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE1 via:read `src/a.rs` — one\nE1 via:read `src/b.rs` — two\n', 'log.md') };
  const { warnings } = await render('# T\n\nSee E1 and E9.\n', { evidence });
  assert.deepEqual(warnings, [
    'evidence E9 has no entry in log.md; shown as plain text — plan.md:3',
    'evidence: 1 warning about log.md itself',
    'evidence: evidence E1 is listed twice; keeping the first — log.md:3',
  ]);
});

test('a ledger citation naming a labelled place in a markdown file links its lines', async () => {
  const root = tempRepo({ files: { 'docs/a.md': '# A\n\n| Tier | Model |\n| --- | --- |\n' }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE1 via:read docs/a.md::Tier table:L3-L4 — tiers\n', 'log.md') };
  const { html, warnings } = await render('# T\n\nSee E1.\n', { roots: [root], evidence });
  assert.deepEqual([/<span class="ev-cite"><a class="path" href="https:\/\/github\.com\/acme\/w\/blob\/[^/]+\/docs\/a\.md#L3-L4">/.test(html), warnings], [true, []]);
});
