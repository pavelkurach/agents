# caller-context-dropped
langs: go

When: `context.Background()`/`TODO()`, or a call lacking a ctx where a `…Context` variant exists, in code that receives a ctx or `*http.Request`.
Ask: Should the work stop when the caller cancels or its deadline passes, and does it need the ctx's values (trace span, auth, request id)? Look for `http.NewRequest`, `db.Query`/`Exec`, `exec.Command` and `net.Dial` where their ctx variants exist. Work that must outlive the request keeps the values without the cancellation through `context.WithoutCancel(ctx)` (Go 1.21+).
Not when: `main`, process-lifetime workers, tests.
Report as: `<call>` at `<site>` ignores the caller's ctx: it runs past cancellation and deadline, holding `<connection / goroutine>` after the client left, and drops trace and auth values; fix: thread `ctx` through and use the `…Context` API.
