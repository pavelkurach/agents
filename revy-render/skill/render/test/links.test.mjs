import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseLinkBase } from '../lib/remote.mjs';
import { commit, edit, headSha, pushed, render, tempRepo } from './helpers.mjs';

const RS = 'pub struct Format {}\n\npub fn check() {}\n';
const github = (sha) => parseLinkBase(`https://github.com/other/x@${sha}`);

test('a link base parses into its forge, project and sha', () => {
  assert.deepEqual(parseLinkBase('https://gitlab.com/team/app@abc123'), { remote: { kind: 'gitlab', host: 'gitlab.com', project: 'team/app' }, ref: 'abc123' });
});

test('a link base without @sha is rejected with the expected form', () => {
  assert.throws(() => parseLinkBase('https://github.com/a/b'), { message: '--link https://github.com/a/b: expected <web-url>@<sha>, e.g. https://github.com/org/repo@1a2b3c4' });
});

test('a link base on an unknown host is rejected', () => {
  assert.throws(() => parseLinkBase('https://example.org/a/b@abc'), { message: '--link https://example.org/a/b@abc: not a GitHub, GitLab or Bitbucket project URL' });
});

test('--link overrides the git remote and ref of a code link', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const sha = headSha(root);
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root], link: github(sha) });
  assert.ok(html.includes(`href="https://github.com/other/x/blob/${sha}/src/a.rs"`));
});

test('a path:L2-L3 reference links the GitHub line range', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const sha = headSha(root);
  const { html } = await render('# T\n\nSee `src/a.rs:L2-L3`.\n', { roots: [root], link: github(sha) });
  assert.ok(html.includes(`/blob/${sha}/src/a.rs#L2-L3"`));
});

test('a path:L2 reference links the one line', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS }, remote: 'git@github.com:acme/w.git' });
  const sha = headSha(root);
  const { html } = await render('# T\n\nSee `src/a.rs:L2`.\n', { roots: [root], link: github(sha) });
  assert.ok(html.includes(`/blob/${sha}/src/a.rs#L2"`));
});

test('a line range on GitLab uses the #L2-3 form', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  const { html } = await render('# T\n\nSee `src/a.rs:L2-L3`.\n', { roots: [root], link: parseLinkBase(`https://gitlab.com/t/app@${sha}`) });
  assert.ok(html.includes(`https://gitlab.com/t/app/-/blob/${sha}/src/a.rs#L2-3"`));
});

test('a line range on Bitbucket uses the #lines-2:3 form', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  const { html } = await render('# T\n\nSee `src/a.rs:L2-L3`.\n', { roots: [root], link: parseLinkBase(`https://bitbucket.org/t/app@${sha}`) });
  assert.ok(html.includes(`https://bitbucket.org/t/app/src/${sha}/src/a.rs#lines-2:3"`));
});

test('a path absent at the link sha is marked local only and not linked', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  commit(root, { 'src/b.rs': RS });
  const { html } = await render('# T\n\nSee `src/b.rs`.\n', { roots: [root], link: github(sha) });
  assert.match(html, /<span class="path path-local"><svg[^]*?<code>src\/<wbr>b\.rs<\/code><span class="chip chip-local">local only<\/span><\/span>/);
});

test('a path whose working tree differs from the link sha links with a local differs marker', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  edit(root, { 'src/a.rs': `${RS}\npub fn more() {}\n` });
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root], link: github(sha) });
  assert.match(html, /<a class="path" href="https:\/\/github\.com\/other\/x\/blob\/[0-9a-f]+\/src\/a\.rs">[^]*?<\/a><span class="chip chip-differs">local differs<\/span>/);
});

test('a path equal at the link sha has no local marker', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const { html } = await render('# T\n\nSee `src/a.rs`.\n', { roots: [root], link: github(headSha(root)) });
  assert.equal(/chip-(?:local|differs)"/.test(html.split('</head>')[1]), false);
});

test('a path::symbol under a link base links the line at the link sha', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  edit(root, { 'src/a.rs': `// moved\n// down\n${RS}` });
  const { html } = await render('# T\n\nSee `src/a.rs::check`.\n', { roots: [root], link: github(sha) });
  assert.ok(html.includes(`/blob/${sha}/src/a.rs#L3"`));
});

test('a link base without --repo links a path-shaped span unverified', async () => {
  const { html } = await render('# T\n\nSee `src/a.rs:L4`.\n', { link: github('abc123') });
  assert.ok(html.includes('href="https://github.com/other/x/blob/abc123/src/a.rs#L4"'));
});

test('a symbol defined only in the working tree links its file, marked local differs, without a warning', async () => {
  const root = tempRepo({ files: { 'src/a.rs': RS } });
  const sha = headSha(root);
  edit(root, { 'src/a.rs': `${RS}\npub fn fresh() {}\n` });
  const { html, warnings } = await render('# T\n\nSee `src/a.rs::fresh`.\n', { roots: [root], link: github(sha) });
  assert.deepEqual({ linked: /\/blob\/[0-9a-f]+\/src\/a\.rs">[^]*?<\/a><span class="chip chip-differs">/.test(html), warnings }, { linked: true, warnings: [] });
});

test('a markdown symbol names the heading it is the start of', async () => {
  const root = tempRepo({ files: { 'docs/fixer.md': '# Fixer\n\nintro\n\n## Walk the findings\n\nsteps\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `docs/fixer.md::Walk`.\n', { roots: [root] });
  assert.match(html, /href="https:\/\/github\.com\/acme\/w\/blob\/main\/docs\/fixer\.md#L5"/);
});

test('a markdown symbol that matches a heading does not warn', async () => {
  const root = tempRepo({ files: { 'docs/fixer.md': '# Fixer\n\n## Walk the findings\n' }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  const { warnings } = await render('# T\n\nSee `docs/fixer.md::Walk`.\n', { roots: [root] });
  assert.deepEqual(warnings, []);
});

test('frontmatter names the front matter block of a markdown file', async () => {
  const root = tempRepo({ files: { 'docs/SKILL.md': '---\nname: x\n---\n# Skill\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `docs/SKILL.md::frontmatter`.\n', { roots: [root] });
  assert.match(html, /href="https:\/\/github\.com\/acme\/w\/blob\/main\/docs\/SKILL\.md#L1"/);
});

test('a markdown label with written lines links those lines', async () => {
  const root = tempRepo({ files: { 'docs/a.md': '# A\n\n| Tier | Model |\n| --- | --- |\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `docs/a.md::Tier:L3-L4`.\n', { roots: [root] });
  assert.match(html, /href="https:\/\/github\.com\/acme\/w\/blob\/main\/docs\/a\.md#L3-L4"/);
});

test('a markdown label with written lines does not warn', async () => {
  const root = tempRepo({ files: { 'docs/a.md': '# A\n\n| Tier | Model |\n| --- | --- |\n' }, remote: 'git@github.com:acme/w.git' });
  pushed(root);
  const { warnings } = await render('# T\n\nSee `docs/a.md::Tier:L3-L4`.\n', { roots: [root] });
  assert.deepEqual(warnings, []);
});

test('a markdown symbol written as a line number links that line', async () => {
  const root = tempRepo({ files: { 'docs/a.md': '# A\n\nx\ny\n' }, remote: 'git@github.com:acme/w.git' });
  const { html } = await render('# T\n\nSee `docs/a.md::L3`.\n', { roots: [root] });
  assert.match(html, /href="https:\/\/github\.com\/acme\/w\/blob\/main\/docs\/a\.md#L3"/);
});
