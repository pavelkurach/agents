# context-used-after-scope-ends
langs: go

When: ctx stored in a struct, returned, handed to a goroutine that outlives the function, or an errgroup's ctx used after `Wait`.
Ask: When is that ctx cancelled — `r.Context()` when the handler returns, a `WithTimeout`/`WithCancel` ctx at its deferred `cancel()`, `errgroup.WithContext`'s when `Wait` returns or the first goroutine fails? Is it used after that point: a background goroutine started from a handler, a lazy iterator, a client built with it?
Report as: `<ctx>` is cancelled at `<point>` but `<work>` uses it afterwards: it fails with `context canceled`, intermittently, once `<the handler returns / Wait ends>`; fix: use the ctx only inside its scope; for deliberate background work, `context.WithoutCancel(ctx)` plus its own timeout.
