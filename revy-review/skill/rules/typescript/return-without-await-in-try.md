# return-without-await-in-try
langs: typescript

When: Promise returned without `await` inside a `try`, a block with `finally`, or a `using` declaration's scope.
Ask: `catch` sees only synchronous throws, and `finally` or disposal runs before the returned promise settles. Is the rejection meant to be caught, or is the lock, connection or span still needed while the promise runs?
Report as: `return <call>` at `<site>` escapes its `try`: rejections skip `<catch>` and `<resource>` is released while still in use; fix: `return await <call>`.
Example:
  bad:  try { return fetchUser(id); } catch { return fallback; }
  good: try { return await fetchUser(id); } catch { return fallback; }
