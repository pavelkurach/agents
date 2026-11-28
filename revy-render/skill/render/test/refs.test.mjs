import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseEvidence } from '../lib/evidence.mjs';
import { parseLinkBase } from '../lib/remote.mjs';
import { render, tempRepo } from './helpers.mjs';

const body = (html) => html.split('</head>')[1];
const LINK = parseLinkBase('https://github.com/acme/w@1111111111111111111111111111111111111111');

test('a link to an id that is not on the page warns', async () => {
  const { warnings } = await render('# T\n\nSee [nowhere](#nowhere).\n');
  assert.deepEqual(warnings, ['link #nowhere matches no id on the page — plan.md:3']);
});

test('a link to an id the page has does not warn', async () => {
  const { warnings } = await render('# T\n\n## Flows\n\nSee [the flows](#flows).\n');
  assert.deepEqual(warnings, []);
});

test('a commit ref links to the commit on the link base host', async () => {
  const { html } = await render('# T\n\nFixed in commit 1a2b3c4d.\n', { link: LINK });
  assert.ok(html.includes('<a class="ref" href="https://github.com/acme/w/commit/1a2b3c4d" title="commit 1a2b3c4d · github.com/acme/w">commit 1a2b3c4d</a>'));
});

test('a PR ref links to the merge request on a GitLab remote', async () => {
  const root = tempRepo({ files: { 'a.txt': 'a' }, remote: 'git@gitlab.com:acme/w.git' });
  const { html } = await render('# T\n\nLanded in PR #12.\n', { roots: [root] });
  assert.ok(html.includes('<a class="ref" href="https://gitlab.com/acme/w/-/merge_requests/12" title="PR #12 · gitlab.com/acme/w">PR #12</a>'));
});

test('an issue ref links to the issue on a Bitbucket remote', async () => {
  const root = tempRepo({ files: { 'a.txt': 'a' }, remote: 'git@bitbucket.org:acme/w.git' });
  const { html } = await render('# T\n\nReported as issue #7.\n', { roots: [root] });
  assert.ok(html.includes('<a class="ref" href="https://bitbucket.org/acme/w/issues/7" title="issue #7 · bitbucket.org/acme/w">issue #7</a>'));
});

test('a history ref in a table cell is linked too', async () => {
  const { html } = await render('# T\n\n| what | where |\n| --- | --- |\n| guard | PR #3 |\n', { link: LINK });
  assert.ok(html.includes('<a class="ref" href="https://github.com/acme/w/pull/3"'));
});

test('a history ref without a host stays plain text', async () => {
  const { html, warnings } = await render('# T\n\nLanded in PR #12.\n');
  assert.deepEqual([body(html).includes('class="ref"'), warnings], [false, []]);
});

test('a commit ref shorter than seven hex digits stays plain text', async () => {
  const { html } = await render('# T\n\nSee commit abc123.\n', { link: LINK });
  assert.equal(body(html).includes('class="ref"'), false);
});

test('a history ref the ledger mentions shows what the ledger says', async () => {
  const evidence = { name: 'log.md', ...parseEvidence('## Evidence\nE2 via:read `src/a.rs` — the guard landed in PR #12\n', 'log.md') };
  const { html } = await render('# T\n\nLanded in PR #12.\n', { link: LINK, evidence });
  assert.ok(html.includes('<a class="ref" href="https://github.com/acme/w/pull/12" title="E2 — the guard landed in PR #12">PR #12</a>'));
});

test('a heading that defines an id gets that id as its anchor', async () => {
  const { html } = await render('# T\n\n## Risks\n\n### F3 — slow store\n\nx\n');
  assert.ok(html.includes('<h3 id="def-F3">F3 — slow store</h3>'));
});

test('a bare id the document defines in a heading links to it', async () => {
  const { html } = await render('# T\n\n## Risks\n\n### F3 — slow store\n\nx\n\n## Plan\n\nThis answers F3.\n');
  assert.ok(html.includes('This answers <a class="xref" href="#def-F3">F3</a>.'));
});

test('a list item opening with a bold id defines it', async () => {
  const { html } = await render('# T\n\n- **R2** — the retry cap\n\nR2 holds.\n');
  assert.ok(html.includes('<li id="def-R2"><strong>R2</strong> — the retry cap</li>'));
});

test('a bare id the document defines in bold links to it', async () => {
  const { html } = await render('# T\n\n- **R2** — the retry cap\n\nR2 holds.\n');
  assert.ok(html.includes('<p><a class="xref" href="#def-R2">R2</a> holds.</p>'));
});

test('an id the document never defines stays plain text', async () => {
  const { html } = await render('# T\n\n- **R2** — the retry cap\n\nF9 holds.\n');
  assert.ok(html.includes('<p>F9 holds.</p>'));
});

test('an id inside a longer word is not linked', async () => {
  const { html } = await render('# T\n\n- **R2** — the retry cap\n\nR2D2 and R2-x.\n');
  assert.ok(html.includes('<p>R2D2 and R2-x.</p>'));
});

const FIGURE = '```mermaid Retry and breaker live inside the API\nflowchart LR\n  a --> b\n```\n';

test('a link to fig- and the opening words of a caption anchors that figure', async () => {
  const { html } = await render(`# T\n\n## A\n\n${FIGURE}\n## B\n\nSee [the figure](#fig-retry-and-breaker).\n`);
  assert.match(html, /<figure id="fig-retry-and-breaker" class="figure diagram wide"/);
});

test('a link to fig- and the opening words of a caption does not warn', async () => {
  const { warnings } = await render(`# T\n\n## A\n\n${FIGURE}\n## B\n\nSee [the figure](#fig-retry-and-breaker).\n`);
  assert.deepEqual(warnings, []);
});

test('a figure no link names gets no id', async () => {
  const { html } = await render(`# T\n\n## A\n\n${FIGURE}`);
  assert.ok(html.includes('<figure class="figure diagram wide"'));
});

test('a link to fig- and words that open no caption warns', async () => {
  const { warnings } = await render(`# T\n\n## A\n\n${FIGURE}\n## B\n\nSee [the figure](#fig-retry-and-b).\n`);
  assert.deepEqual(warnings, ['link #fig-retry-and-b matches no id on the page — plan.md:12']);
});

test('a section title naming code shows that code as code in the navigation', async () => {
  const { html } = await render('# T\n\n## The `--json` flag\n\nx\n\n## Notes\n\ny\n');
  assert.ok(html.includes('<li><a href="#the-json-flag">The <code class="code">--json</code> flag</a></li>'));
});
