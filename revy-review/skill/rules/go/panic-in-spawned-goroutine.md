# panic-in-spawned-goroutine
langs: go

When: Goroutine started (`go`, `errgroup.Go`, worker pool) in a server, consumer or library, with a body that can panic (nil, index, assertion, callback).
Ask: Does the goroutine recover itself? A panic in any goroutine ends the process; `net/http`'s per-handler recovery, recover middleware and the parent's deferred `recover` cover only their own goroutine.
Not when: crashing is the intended policy (`main`-level workers under a supervisor).
Report as: a panic in the goroutine at `<site>` is unrecovered: one bad `<input>` kills the whole `<server>` for every client; fix: a deferred recover inside the goroutine that reports the failure to its owner, or remove the panic source.
