# poll-waker-contract
langs: rust

When: `Future::poll` or `Stream::poll_next` implemented by hand, or an executor, poll loop or timer added.
Ask: On `Pending`, is the current waker registered with the event source that will fire, before the last readiness check, so an event arriving in between still wakes the task? Is the waker from the latest poll stored (the task can move; compare with `will_wake`)? Calling `wake_by_ref()` right before `Pending`, re-polling every task each tick, or checking `Instant::now()` on each poll spins instead of waiting.
Not when: one cooperative yield after real progress, under a budget.
Report as: `<future>` returns `Pending` `<without registering the waker / registering it after the check / waking itself>`: the task `<hangs when the wakeup is lost / spins a core at 100% while idle>`; fix: register the waker with the source (`AsyncFd`, a channel, `tokio::time::sleep`) before the final check.
