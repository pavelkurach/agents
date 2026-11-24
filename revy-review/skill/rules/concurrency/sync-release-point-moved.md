# sync-release-point-moved
langs: any

When: A barrier, latch, gate, condition variable, semaphore, countdown or start signal is replaced, or its wait or release moves to another point.
Ask: What event does each waiter wait for in base, and what is true by the time it is released (all threads running, all work queued, a resource held)? Does head release at the same point, or earlier? A barrier releases when every participant has called `wait`, so every thread is running; `spawn` returning means only that the thread exists, so a flag set after the spawn loop releases before the threads have started. An earlier release can still be correct yet serialise or stagger the work that base started together; compare start skew and total time on the real work, not a stubbed copy.
Not when: the release point is unchanged and only the primitive's type changed.
Report as: `<primitive>` now releases `<waiters>` when `<head event>`, not when `<base event>`: `<consequence: work starts staggered / races ahead of setup / runs serially>`; fix: release on the base event (wait until every `<participant>` has arrived), or keep base's primitive.
