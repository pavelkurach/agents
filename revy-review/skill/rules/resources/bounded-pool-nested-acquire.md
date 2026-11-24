# bounded-pool-nested-acquire
langs: any

When: Connection, session, permit or executor task acquired and waited on inside code that already holds one from the same bounded pool.
Ask: Find the nesting: a request holding a DB connection calls a helper that checks out its own session; a task on a fixed executor submits a child to the same executor and blocks on its result; a semaphore holder awaits work that needs a permit. Once every slot is held by a waiter (concurrency ≥ pool size), nothing finishes. Compare the pool size with peak concurrency in the deploy config.
Report as: `<outer>` holds a `<pool>` slot while waiting for a second from the same pool: at `<pool size>` concurrent `<requests / tasks>` every slot waits and the service deadlocks until timeouts fire; fix: pass the held connection or session down, run children on a separate pool, or acquire once at the top.
