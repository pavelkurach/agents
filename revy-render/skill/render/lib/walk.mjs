// The order a flowchart or state diagram is stepped through on the page. Pure: edges in, steps out.

/**
 * An edge as drawn: `failure` when it or the node it enters is classed failure, `ok` when it is classed ok.
 * @typedef {{ id: string, from: string, to: string, kind: 'ok' | 'plain' | 'failure' }} WalkEdge
 */
/**
 * One step: the node reached, the edge taken to reach it (none for a path's first node), and the path it belongs to
 * (0 is the main path; each later number one failure or side path).
 * @typedef {{ node: string, edge?: string, path: number }} Step
 */

const RANK = { ok: 0, plain: 1, failure: 2 };

/**
 * The main path first — from the node no edge enters, along ok then plain edges — then each failure edge's path in
 * declaration order, then whatever is left (side inputs, other branches). Every edge is walked once; a path ends at
 * a node already walked or where no unwalked ok or plain edge leaves.
 * @param {WalkEdge[]} edges  in declaration order
 * @returns {Step[]}
 */
export function walkOrder(edges) {
  if (!edges.length) return [];
  const entered = new Set(edges.map((e) => e.to));
  const start = (edges.find((e) => !entered.has(e.from)) ?? edges[0]).from;
  const steps = [{ node: start, path: 0 }];
  const walked = new Set([start]);
  const used = new Set();
  const take = (e, path) => {
    used.add(e.id);
    steps.push({ node: e.to, edge: e.id, path });
    if (walked.has(e.to)) return;
    walked.add(e.to);
    const onward = edges.filter((x) => x.from === e.to && x.kind !== 'failure' && !used.has(x.id))
      .sort((a, b) => RANK[a.kind] - RANK[b.kind])[0];
    if (onward) take(onward, path);
  };
  const first = edges.filter((e) => e.from === start && e.kind !== 'failure').sort((a, b) => RANK[a.kind] - RANK[b.kind])[0];
  if (first) take(first, 0);
  for (let path = 1; used.size < edges.length; path += 1) {
    const open = edges.filter((e) => !used.has(e.id));
    const next = open.find((e) => e.kind === 'failure' && walked.has(e.from)) ?? open.find((e) => walked.has(e.from)) ?? open[0];
    if (!walked.has(next.from)) {
      steps.push({ node: next.from, path });
      walked.add(next.from);
    }
    take(next, path);
  }
  return steps;
}

/**
 * The author's order: each listed node, reached by the edge from the node listed just before it (same path), else by
 * an edge from the latest earlier listed node (a new path), else by no edge (a new path).
 * @param {WalkEdge[]} edges
 * @param {string[]} ids  node ids, each known to the diagram
 * @returns {Step[]}
 */
export function pinnedOrder(edges, ids) {
  let path = 0;
  return ids.map((node, i) => {
    if (i === 0) return { node, path };
    const from = ids.slice(0, i).reverse().find((id) => edges.some((e) => e.from === id && e.to === node));
    if (from !== ids[i - 1]) path += 1;
    const e = from === undefined ? undefined : edges.find((x) => x.from === from && x.to === node);
    return e ? { node, edge: e.id, path } : { node, path };
  });
}
