import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const CLI = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'render.mjs');
const dir = () => mkdtempSync(path.join(tmpdir(), 'revy-cli-'));
const file = (where, name, text) => {
  const p = path.join(where, name);
  writeFileSync(p, text);
  return p;
};
const run = (...args) => spawnSync(process.execPath, [CLI, ...args], { encoding: 'utf8' });

test('a render writes plan.html, prints its path and exits 0', () => {
  const d = dir();
  const r = run('--plan', file(d, 'plan.md', '# T\n'), '--out', path.join(d, 'out'));
  assert.deepEqual({ code: r.status, out: r.stdout }, { code: 0, out: `wrote ${path.join(d, 'out', 'plan.html')}\n` });
});

test('a render leaves only plan.html in the output directory', () => {
  const d = dir();
  run('--plan', file(d, 'plan.md', '# T\n'), '--out', path.join(d, 'out'));
  assert.deepEqual(readdirSync(path.join(d, 'out')), ['plan.html']);
});

test('a missing --out exits 1 with the usage', () => {
  const d = dir();
  const r = run('--plan', file(d, 'plan.md', '# T\n'));
  assert.deepEqual({ code: r.status, first: r.stderr.split('\n')[0] }, { code: 1, first: 'error: --plan and --out are required' });
});

test('an unreadable plan exits 1 naming the file', () => {
  const d = dir();
  const r = run('--plan', path.join(d, 'nope.md'), '--out', d);
  assert.deepEqual({ code: r.status, err: r.stderr }, { code: 1, err: `error: cannot read plan ${path.join(d, 'nope.md')}: ENOENT\n` });
});

test('a look file that is not JSON exits 1 naming the file', () => {
  const d = dir();
  const look = file(d, 'look.json', '{ order: ');
  const r = run('--plan', file(d, 'plan.md', '# T\n'), '--look', look, '--out', d);
  assert.deepEqual({ code: r.status, starts: r.stderr.startsWith(`error: look file ${look} is not valid JSON: `) }, { code: 1, starts: true });
});

test('a look file with a wrongly typed field exits 1 naming the field', () => {
  const d = dir();
  const r = run('--plan', file(d, 'plan.md', '# T\n'), '--look', file(d, 'look.json', '{"fold": "Impact"}'), '--out', d);
  assert.deepEqual({ code: r.status, err: r.stderr }, { code: 1, err: 'error: look.fold must be an array of strings\n' });
});

test('a copy that differs from its --source exits 1 with the first differing line and writes nothing', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n\nGoal: ship.\n');
  const copy = file(d, 'render.md', '# T\n\nGoal: ship it.\n');
  const r = run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.deepEqual(
    { code: r.status, err: r.stderr, wrote: existsSync(path.join(d, 'out', 'plan.html')) },
    { code: 1, err: `error: ${copy}:3 differs from ${source}:3 outside the inserted figures:\n  copy:   Goal: ship it.\n  source: Goal: ship.\n`, wrote: false },
  );
});

test('a copy that only adds a figure to its --source renders', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n\nGoal: ship.\n');
  const copy = file(d, 'render.md', '# T\n\nGoal: ship.\n\n```steps\nsteps: [{ node: a, verb: then }, b]\n```\n');
  const r = run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.equal(r.status, 0);
});

test('a render writes the page under the input basename', () => {
  const d = dir();
  const r = run('--plan', file(d, 'explanation.md', '# T\n'), '--out', path.join(d, 'out'));
  assert.equal(r.stdout, `wrote ${path.join(d, 'out', 'explanation.html')}\n`);
});

test('a render with --source names the page after the source', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n\nGoal: ship.\n');
  const copy = file(d, 'render.md', '# T\n\nGoal: ship.\n');
  const r = run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.equal(r.stdout, `wrote ${path.join(d, 'out', 'plan.html')}\n`);
});

test('a warning from a copy with an inserted figure cites the source file and line', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n\nGoal: ship.\n\n```\nplain\n```\n');
  const copy = file(d, 'render.md', '# T\n\n```steps\nsteps: [{ node: a, verb: then }, b]\n```\n\nGoal: ship.\n\n```\nplain\n```\n');
  const r = run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.equal(r.stdout.split('\n')[0], 'warn: code block has no language; shown plain — plan.md:5');
});

test('a warning inside an inserted figure cites the copy', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n');
  const copy = file(d, 'render.md', '# T\n\n```tree\nfiles:\n  - path: a.rs\n    status: addded\n```\n');
  const r = run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.match(r.stdout.split('\n')[0], /— render\.md:\d+$/);
});

test('the header names the source file, not the copy', () => {
  const d = dir();
  const source = file(d, 'plan.md', '# T\n');
  const copy = file(d, 'render.md', '# T\n');
  run('--plan', copy, '--source', source, '--out', path.join(d, 'out'));
  assert.match(readFileSync(path.join(d, 'out', 'plan.html'), 'utf8'), /<p class="eyebrow">plan\.md · /);
});

test('a --link sha the --repo does not have exits 1 naming both', () => {
  const d = dir();
  const r = run('--plan', file(d, 'plan.md', '# T\n'), '--out', d, '--repo', path.dirname(CLI), '--link', 'https://github.com/a/b@0000000000000000000000000000000000000001');
  assert.deepEqual({ code: r.status, err: r.stderr }, { code: 1, err: `error: --link sha 0000000000000000000000000000000000000001 is not a commit in --repo ${path.dirname(CLI)}; fetch it or pick a sha the repo has\n` });
});

test('--lint of a clean document prints nothing, exits 0 and writes nothing', () => {
  const d = dir();
  const r = run('--lint', '--plan', file(d, 'plan.md', '# T\n\nok\n'));
  assert.deepEqual({ code: r.status, out: r.stdout, files: readdirSync(d) }, { code: 0, out: '', files: ['plan.md'] });
});

test('--lint prints warnings as warn lines and exits 0', () => {
  const d = dir();
  const r = run('--lint', '--plan', file(d, 'plan.md', '# T\n\n```\nplain\n```\n'));
  assert.deepEqual({ code: r.status, out: r.stdout }, { code: 0, out: 'warn: code block has no language; shown plain — plan.md:3\n' });
});

test('--lint reports a diagram that cannot be drawn as an error and exits 1', () => {
  const d = dir();
  const r = run('--lint', '--plan', file(d, 'plan.md', '# T\n\n```mermaid\nflowchart LR\n  A --> \n```\n'));
  assert.deepEqual({ code: r.status, error: /^error: Mermaid .* — plan\.md:5$/m.test(r.stdout) }, { code: 1, error: true });
});

test('--lint reports a component that cannot be drawn as an error and exits 1', () => {
  const d = dir();
  const r = run('--lint', '--plan', file(d, 'plan.md', '# T\n\n```tree\nfiles: nope\n```\n'));
  assert.deepEqual({ code: r.status, error: /^error: tree: not drawn; shown as source — plan\.md:3$/m.test(r.stdout) }, { code: 1, error: true });
});

test('--lint checks evidence ids against the --evidence file', () => {
  const d = dir();
  const r = run('--lint', '--plan', file(d, 'plan.md', '# T\n\nSee E2.\n'), '--evidence', file(d, 'log.md', '## Evidence\nE1 via:read a — b\n'));
  assert.equal(r.stdout, 'warn: evidence E2 has no entry in log.md; shown as plain text — plan.md:3\n');
});
