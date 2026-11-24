# cancellation-leaves-partial-state
langs: any

When: Cleanup or the second half of a state change placed after an `await`, or async work wrapped in a timeout, race, `select!` or drop-on-disconnect.
Ask: Cancellation lands at any await: `asyncio.timeout`/`wait_for`, a failing `TaskGroup` sibling, `select!`, `tokio::time::timeout`, `AbortSignal`, or a server dropping the handler future when the client disconnects. At each await, what does cancellation leave — a held resource, a half-applied update, lost buffered data? Does cleanup run in `finally`, `Drop` or `defer`, and does Python code re-raise `CancelledError` after it? Is must-finish work shielded or safe to redo? Is each `select!` branch cancel-safe per its docs (`read_exact` and `read_line` are not)?
Report as: cancellation at `<await>` in `<fn>` leaves `<partial state>`; fix: cleanup in `finally` or a `Drop` guard, one atomic step, or shield the must-complete part.
