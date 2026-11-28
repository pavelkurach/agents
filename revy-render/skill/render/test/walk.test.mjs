// The order a flowchart or state diagram is stepped through: derived from its edges, or pinned by the author.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { pinnedOrder, walkOrder } from '../lib/walk.mjs';

const edge = (id, from, to, kind = 'plain') => ({ id, from, to, kind });

test('a chain is walked node by node from its start', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'B', 'C')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'C', edge: 'e2', path: 0 }]);
});

test('the walk starts at the node no edge enters, even when it is declared later', () => {
  const steps = walkOrder([edge('e1', 'B', 'C'), edge('e2', 'A', 'B')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e2', path: 0 }, { node: 'C', edge: 'e1', path: 0 }]);
});

test('the main path takes an ok edge before a plain one', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'A', 'C', 'ok')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'C', edge: 'e2', path: 0 }, { node: 'B', edge: 'e1', path: 1 }]);
});

test('a failure edge is walked after the main path even when declared first', () => {
  const steps = walkOrder([edge('e1', 'A', 'F', 'failure'), edge('e2', 'A', 'B')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e2', path: 0 }, { node: 'F', edge: 'e1', path: 1 }]);
});

test('a failure path continues along the plain edges past its failure edge', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'A', 'F', 'failure'), edge('e3', 'F', 'G')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'F', edge: 'e2', path: 1 }, { node: 'G', edge: 'e3', path: 1 }]);
});

test('an edge back to a node already walked ends its path', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'B', 'A'), edge('e3', 'A', 'C', 'failure')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'A', edge: 'e2', path: 0 }, { node: 'C', edge: 'e3', path: 1 }]);
});

test('each failure edge gets its own path, in declaration order', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'B', 'G', 'failure'), edge('e3', 'A', 'F', 'failure')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'G', edge: 'e2', path: 1 }, { node: 'F', edge: 'e3', path: 2 }]);
});

test('a second start node is walked after the failure paths', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'W', 'B'), edge('e3', 'A', 'F', 'failure')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'F', edge: 'e3', path: 1 }, { node: 'W', path: 2 }, { node: 'B', edge: 'e2', path: 2 }]);
});

test('a graph that is one cycle starts at its first edge', () => {
  const steps = walkOrder([edge('e1', 'A', 'B'), edge('e2', 'B', 'A')]);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'A', edge: 'e2', path: 0 }]);
});

test('a graph with no edges has no steps', () => {
  assert.deepEqual(walkOrder([]), []);
});

test('a pinned order follows the listed nodes along the edges between them', () => {
  const steps = pinnedOrder([edge('e1', 'A', 'B'), edge('e2', 'A', 'C'), edge('e3', 'C', 'B')], ['A', 'C', 'B']);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'C', edge: 'e2', path: 0 }, { node: 'B', edge: 'e3', path: 0 }]);
});

test('a pinned node not reached from the one before starts a new path from an earlier listed node', () => {
  const steps = pinnedOrder([edge('e1', 'A', 'B'), edge('e2', 'A', 'C')], ['A', 'B', 'C']);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'B', edge: 'e1', path: 0 }, { node: 'C', edge: 'e2', path: 1 }]);
});

test('a pinned node no listed node leads to starts a new path on its own', () => {
  const steps = pinnedOrder([edge('e1', 'A', 'B')], ['A', 'W']);
  assert.deepEqual(steps, [{ node: 'A', path: 0 }, { node: 'W', path: 1 }]);
});
