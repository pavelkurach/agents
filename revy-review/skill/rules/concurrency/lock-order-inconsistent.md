# lock-order-inconsistent
langs: any

When: Lock (mutex, DB row, advisory lock) acquired while another is held.
Ask: Find every other acquire site of the same locks: does any take them in the opposite order? Are multi-row locks taken in a sorted order (`ORDER BY id … FOR UPDATE`, sorted keys)? Does a callee re-take a non-reentrant lock its caller holds?
Report as: `<path A>` takes `<L1>`→`<L2>` while `<path B>` takes `<L2>`→`<L1>` (or `<fn>` re-locks `<L>`): deadlock under concurrency; fix: one global lock order, or one lock.
