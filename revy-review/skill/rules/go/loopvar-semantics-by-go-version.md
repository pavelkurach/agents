# loopvar-semantics-by-go-version
langs: go

When: Loop variable captured in a closure or goroutine, `&v` kept from a loop, or the `go.mod` `go` line or a `//go:build go1.N` line changed.
Ask: Which language version does this file compile at — the `go.mod` `go` line, lowered by a `//go:build go1.N` in the file? Below 1.22 all iterations share one variable, and vet flags only a `go`, `defer` or `t.Run` closure that is the loop body's last statement. When a bump crosses 1.22, does a loop rely on the shared variable?
Report as: closure or pointer at `<site>` sees `<the last element / one shared variable>` because `<file>` compiles at go `<version>`: `<every goroutine handles the same item>`; fix: a per-iteration copy (`v := v`), or move the module to go 1.22+.
