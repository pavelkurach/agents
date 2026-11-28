import assert from 'node:assert/strict';
import { test } from 'node:test';
import { render } from './helpers.mjs';

const wrapOf = (html) => /<div class="scroll wide table-wrap"[^>]*>/.exec(html)[0];

test('path and status columns are marked as short columns', async () => {
  const { html } = await render('# T\n\n## Changes\n\n| File | Status | What changes |\n| --- | --- | --- |\n| `src/a.rs` | added | the retry loop that wraps the store call |\n| `src/b.rs` | modified | the entry point that wires the guard in |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap" data-short="1 2">');
});

test('a column of numbers is a short column', async () => {
  const { html } = await render('# T\n\n| Step | Took |\n| --- | --- |\n| the first attempt times out after the full budget | 2000 ms |\n| the retry succeeds | 200 ms |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap" data-short="2">');
});

test('a column of ids is a short column', async () => {
  const { html } = await render('# T\n\n| Id | Risk |\n| --- | --- |\n| R1 | all replicas probe at the same moment |\n| R2 | a timed-out success is written twice |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap" data-short="1">');
});

test('a column with one sentence in it is a prose column', async () => {
  const { html } = await render('# T\n\n| Lane | Why |\n| --- | --- |\n| api | affected |\n| web | the bundle grows by the client code |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap" data-short="1">');
});

test('empty cells leave a short column short', async () => {
  const { html } = await render('# T\n\n| Id | Risk |\n| --- | --- |\n| R1 | all replicas probe at the same moment |\n|  | a timed-out success is written twice |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap" data-short="1">');
});

test('a table of prose columns only keeps its plain frame', async () => {
  const { html } = await render('# T\n\n| Question | Answer |\n| --- | --- |\n| how does the server learn it | from the header byte |\n');
  assert.equal(wrapOf(html), '<div class="scroll wide table-wrap">');
});
