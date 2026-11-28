import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render, tempRepo } from './helpers.mjs';

const body = (html) => html.split('</head>')[1];
const HUNK = '@@ -38,5 +40,7 @@ fn index(...)\n fn index() {\n-    let a = 1;\n-    let b = 2;\n+    let a = one();\n+    let b = two();\n+    guard(a)?;\n+    check(b)?;\n     done();\n }\n';
const DIFF = `\`\`\`diff src/rules.rs E7\n${HUNK}\`\`\`\n`;
/** The text of the line with this id, tags stripped. */
const lineText = (html, id) => new RegExp(`id="${id}"[^>]*>(.*?)</span>\\n`).exec(html)[1].replace(/<[^>]+>/g, '');

test('a located diff gives a removed line its base number and a minus sign', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.ok(html.includes('<span class="line diff-del" id="snip-3-B39" data-base="39" data-gutter="39    −">'));
});

test('a located diff gives an added line its head number and a plus sign', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.ok(html.includes('<span class="line diff-add" id="snip-3-L43" data-line="43" data-gutter="   43 +">'));
});

test('a located diff gives a context line both numbers', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.ok(html.includes('<span class="line" id="snip-3-L40" data-line="40" data-base="38" data-gutter="38 40  ">'));
});

test('a located diff shows each hunk header as its own row', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.ok(html.includes('<code><span class="line diff-hunk">@@ -38,5 +40,7 @@ fn index(...)</span>\n'));
});

test('a located diff line shows its code without the diff marker', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.equal(lineText(html, 'snip-3-B39'), '    let a = 1;');
});

test('a located diff highlights its code in the language of the path', async () => {
  const { html } = await render(`# T\n\n${DIFF}`);
  assert.match(html, /id="snip-3-B39"[^>]*><span style="--shiki-light:#D73A49;--shiki-dark:#F97583">    let<\/span>|id="snip-3-B39"[^>]*><span[^>]*>    <\/span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583">let<\/span>/);
});

test('a later hunk numbers its lines from its own header', async () => {
  const { html } = await render(`# T\n\n\`\`\`diff src/rules.rs\n${HUNK}@@ -90,1 +94,1 @@\n-old();\n+new();\n\`\`\`\n`);
  assert.ok(html.includes('<span class="line diff-add" id="snip-3-L94" data-line="94" data-gutter="   94 +">'));
});

test('a head range note makes a chunk of those head lines', async () => {
  const { html } = await render(`# T\n\n${DIFF}\n- L43-L44 — the new guards\n`);
  assert.ok(html.includes('<span class="chunk" tabindex="0" role="group" aria-label="Lines 43–44" aria-describedby="snip-3-n1" data-note="snip-3-n1"><span class="line diff-add" id="snip-3-L43"'));
});

test('a base range note makes a chunk of those base lines', async () => {
  const { html } = await render(`# T\n\n${DIFF}\n- -L39-L40 — the old setup\n`);
  assert.ok(html.includes('<span class="chunk" tabindex="0" role="group" aria-label="Base lines 39–40" aria-describedby="snip-3-n1" data-note="snip-3-n1"><span class="line diff-del" id="snip-3-B39"'));
});

test('a base note names its side and links its first base line', async () => {
  const { html } = await render(`# T\n\n${DIFF}\n- -L39-L40 — the old setup\n`);
  assert.ok(html.includes('<li id="snip-3-n1" data-lines="39-40" data-side="base"><a class="ln" href="#snip-3-B39">-L39-L40</a> the old setup</li>'));
});

test('head and base notes on one hunk make separate chunks that alternate where they touch', async () => {
  const { html } = await render(`# T\n\n${DIFF}\n- -L39-L40 — the old setup\n- L41-L42 — the new setup\n- L43-L44 — the new guards\n`);
  assert.deepEqual([...body(html).matchAll(/<span class="(chunk[^"]*)"[^>]*aria-label="([^"]+)"/g)].map((m) => `${m[1]} ${m[2]}`), ['chunk Base lines 39–40', 'chunk alt Lines 41–42', 'chunk Lines 43–44']);
});

test('lines outside every noted chunk of a diff stay plain', async () => {
  const { html } = await render(`# T\n\n${DIFF}\n- L43-L44 — the new guards\n`);
  assert.ok(html.includes('</span>\n<span class="line" id="snip-3-L40" data-line="40"'));
});

test('a head note naming lines outside the hunks warns', async () => {
  const { warnings } = await render(`# T\n\n${DIFF}\n- L50-L52 — not shown\n`);
  assert.deepEqual(warnings, ['diff note L50-L52 names head lines outside the hunks (40-46) — plan.md:16']);
});

test('a base note naming lines outside the hunks warns', async () => {
  const { warnings } = await render(`# T\n\n${DIFF}\n- -L45 — not shown\n`);
  assert.deepEqual(warnings, ['diff note -L45 names base lines outside the hunks (38-42) — plan.md:16']);
});

test('a malformed hunk header warns', async () => {
  const { warnings } = await render('# T\n\n```diff src/rules.rs\n@@ -38 +40,x @@\n-a\n+b\n```\n');
  assert.deepEqual(warnings, ['diff hunk header "@@ -38 +40,x @@" is malformed; expected "@@ -<line>,<count> +<line>,<count> @@"; shown as a plain diff — plan.md:3']);
});

test('a diff with a malformed hunk header is shown as a plain diff', async () => {
  const { html } = await render('# T\n\n```diff src/rules.rs\n@@ -38 +40,x @@\n-a\n+b\n```\n');
  assert.equal(body(html).includes('data-gutter'), false);
});

test('a hunk whose line counts disagree with its header warns', async () => {
  const { warnings } = await render(`# T\n\n\`\`\`diff src/rules.rs\n${HUNK.replace('-38,5', '-38,6')}\`\`\`\n`);
  assert.deepEqual(warnings, ['diff hunk "@@ -38,6 +40,7 @@" has 5 base and 7 head lines; its header says 6 and 7 — plan.md:3']);
});

test('a located diff cites its path as a link to the file', async () => {
  const root = tempRepo({ files: { 'src/rules.rs': 'x\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(`# T\n\n${DIFF}`, { roots: [root] });
  assert.match(html, /<figcaption><a class="path cite" href="https:\/\/github\.com\/acme\/w\/blob\/main\/src\/rules\.rs">[^]*?<\/a> E7<\/figcaption>/);
});

test('a PR in a located diff info links the location to the PR files view', async () => {
  const root = tempRepo({ files: { 'src/rules.rs': 'x\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render(`# T\n\n\`\`\`diff src/rules.rs PR #12 E7\n${HUNK}\`\`\`\n`, { roots: [root] });
  assert.match(html, /<figcaption><a class="path cite" href="https:\/\/github\.com\/acme\/w\/pull\/12\/files">[^]*?<\/a> <a class="ref"[^>]*>PR #12<\/a> E7<\/figcaption>/);
});

test('a PR in a located diff info links to the merge request diffs on GitLab', async () => {
  const root = tempRepo({ files: { 'src/rules.rs': 'x\n' }, remote: 'git@gitlab.com:acme/w.git' });
  const { html } = await render(`# T\n\n\`\`\`diff src/rules.rs PR #12\n${HUNK}\`\`\`\n`, { roots: [root] });
  assert.ok(html.includes('<a class="path cite" href="https://gitlab.com/acme/w/-/merge_requests/12/diffs">'));
});

test('a diff fence without a path stays a plain diff', async () => {
  const { html } = await render(`# T\n\n\`\`\`diff\n${HUNK}\`\`\`\n`);
  assert.equal(body(html).includes('data-gutter'), false);
});
