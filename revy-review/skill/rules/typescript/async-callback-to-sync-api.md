# async-callback-to-sync-api
langs: typescript

When: `async` or Promise-returning callback passed to an API that ignores its result: `forEach`, `filter`, `some`, `find`, `sort`, listeners, timers.
Ask: Who awaits the callback's promise? `forEach`, listeners and timers drop it: the caller moves on before the work ends, and a rejection goes unhandled, which exits Node 15+. `filter`, `some` and `find` test the always-truthy Promise; an async `sort` comparator returns no number; an async `reduce` must await its accumulator. Express 4 ignores rejected handler promises (Express 5 forwards them); `EventEmitter` routes listener rejections to `'error'` only with `captureRejections`.
Report as: async callback to `<api>` at `<site>` is never awaited: `<the function returns before the work finishes / every item passes the filter / a rejection crashes the process>`; fix: `for…of` with `await`, or `await Promise.all(items.map(…))`; wrap listeners and handlers to catch and forward errors.
Example:
  bad:  items.forEach(async (i) => { await save(i); }); return "done";
  good: for (const i of items) await save(i);
