import assert from 'node:assert/strict';
import { test } from 'node:test';
import { copyDifference } from '../lib/verbatim.mjs';

const SOURCE = '# T\n\n## Delivery\n\n- slice one\n- slice two\n';

test('a copy that only inserts a component is the source', () => {
  const copy = '# T\n\n## Delivery\n\n```timeline\nsteps:\n  - title: one\n```\n\n- slice one\n- slice two\n';
  assert.equal(copyDifference(copy, SOURCE), undefined);
});

test('a copy that inserts a mermaid figure the source lacks is the source', () => {
  const copy = '# T\n\n## Delivery\n\n- slice one\n- slice two\n\n```mermaid One before two\nflowchart LR\n  A --> B\n```\n';
  assert.equal(copyDifference(copy, SOURCE), undefined);
});

test('a reworded line is reported with both line numbers and texts', () => {
  const copy = '# T\n\n## Delivery\n\n- slice one\n- the second slice\n';
  assert.deepEqual(copyDifference(copy, SOURCE), { copyLine: 6, copyText: '- the second slice', sourceLine: 6, sourceText: '- slice two' });
});

test('a dropped line is reported as missing from the copy', () => {
  const copy = '# T\n\n## Delivery\n\n- slice one\n';
  assert.deepEqual(copyDifference(copy, SOURCE), { copyLine: 6, copyText: undefined, sourceLine: 6, sourceText: '- slice two' });
});

test('a code fence of the source changed in the copy is reported at its first differing line', () => {
  const source = '# T\n\n```rust\nfn a() {}\n```\n';
  const copy = '# T\n\n```rust\nfn b() {}\n```\n';
  assert.deepEqual(copyDifference(copy, source), { copyLine: 4, copyText: 'fn b() {}', sourceLine: 4, sourceText: 'fn a() {}' });
});

test('a source mermaid diagram restyled with a caption and kind classes is the source', () => {
  const source = '# T\n\n```mermaid\nflowchart LR\n  A --> B\n```\n';
  const copy = '# T\n\n```mermaid A feeds B\nflowchart LR\n  A:::new e1@--> B\n  class e1 new\n```\n';
  assert.equal(copyDifference(copy, source), undefined);
});

test('a source mermaid diagram whose edges changed in the copy is reported', () => {
  const source = '# T\n\n```mermaid\nflowchart LR\n  A --> B\n```\n';
  const copy = '# T\n\n```mermaid\nflowchart LR\n  A --> C\n```\n';
  assert.deepEqual(copyDifference(copy, source), { copyLine: 5, copyText: '  A --> C', sourceLine: 5, sourceText: '  A --> B' });
});
