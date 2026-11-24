# defer-captures-stale-value
langs: go

When: `defer f(x, …)` with arguments, or a deferred closure that reads `err` or a result to decide on rollback, logging or wrapping.
Ask: Arguments are evaluated at the `defer` line, so `defer finish(err)` gets `err` as it was then, usually `nil`. Does the closure read a named result? With unnamed results, or an inner `x, err := …` shadowing the outer `err`, it reads a local that `return x, fmt.Errorf(…)` never sets.
Report as: deferred `<call>` at `<site>` reads `<value>` from `<the defer line / a shadowed variable>`: `<rollback skipped on error / failure never logged>`; fix: a named result (`(err error)`) read inside the deferred closure, with no inner `:=` shadowing it.
Example:
  bad:  defer finish(err)
  good: defer func() { finish(err) }()   // err is the named result
