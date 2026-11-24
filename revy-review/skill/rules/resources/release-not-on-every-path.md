# release-not-on-every-path
langs: any

When: File, connection, cursor, response body, transaction, lock or subscription released explicitly, an exit added before its release, or `defer` in a loop.
Ask: Does every exit between acquire and release — return, raise or `?`, `break`, cancellation — release it, and does a caller that takes ownership release it? Go: once `err == nil`, is `resp.Body` closed on every path, non-2xx and decode errors included, and drained (`io.Copy(io.Discard, resp.Body)`) where the connection should be reused? A `defer` inside a loop runs at function return, so every iteration holds its resource until then, and a deferred `Unlock` deadlocks on the next `Lock`. Rust: does `mem::forget` or `Box::leak` skip a needed `Drop`?
Report as: `<resource>` from `<site>` stays held on `<exit path / until fn returns>`: under errors or large inputs it piles up until `<pool / fd>` exhaustion or deadlock; fix: scoped acquisition (`with`, RAII, `try/finally`, `defer` inside a per-iteration function).
