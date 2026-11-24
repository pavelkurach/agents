# promise-rejects-before-await
langs: typescript

When: Several promises started and awaited one at a time later, or a promise kept to await after other awaits.
Ask: Can one reject while the code waits on an earlier one? With no handler attached yet, Node reports an unhandled rejection and (by default since Node 15) exits the process, although a later `await` would have caught it.
Report as: `<promise>` at `<site>` can reject before it is awaited: `<failure>` crashes the process through an unhandled rejection; fix: `await Promise.all([...])` or `allSettled` over the started promises, or await each where it is created.
Example:
  bad:  const u = getUser(); const o = getOrders(); await u; await o;
  good: const [u, o] = await Promise.all([getUser(), getOrders()]);
