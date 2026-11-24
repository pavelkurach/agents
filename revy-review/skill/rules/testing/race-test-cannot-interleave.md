# race-test-cannot-interleave
langs: any

When: Test added or changed for a race, deadlock, locking, ordering or idempotency fix.
Ask: Can the test produce the bad interleaving? Calls made one after another cannot, nor can a single-threaded runtime (`#[tokio::test]` defaults to `current_thread`; asyncio switches only at `await`), nor two actors without a barrier holding both past the check before either writes. Would it fail on every run with the fix reverted? Does it assert the end state (one winner, no duplicate row), beyond finishing within a timeout or passing N times?
Report as: `<test>` never interleaves `<actors>`, so it passes with the race present; fix: start them concurrently behind a barrier at `<point>` (or model-check memory ordering with `loom`) and assert `<end state>`.
