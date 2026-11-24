# wall-clock-duration
langs: any

When: Elapsed time, timeout or rate window computed by subtracting wall-clock readings (`time.time()`, `datetime.now()`, `SystemTime`, `Date.now()`).
Ask: Does the result drive a timeout, expiry, retry or throttle that a clock step (NTP correction, VM resume, manual change) would break?
Not when: Go `time.Since` on `time.Now()` values (the monotonic reading is included) · deadlines shared across processes.
Report as: `<duration>` uses the wall clock: a clock step makes it negative or jump, so `<the timeout misfires>`; fix: `time.monotonic()`, `Instant`, `performance.now()`.
