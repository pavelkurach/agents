# time-compared-with-equals
langs: go

When: `time.Time` compared with `==`/`!=`, used as a map key or inside a struct compared with `==`, or tested with `t == time.Time{}`.
Ask: `==` compares the location and the monotonic reading as well as the instant, so one instant from `time.Now()`, a DB or JSON round trip, or `.UTC()` compares unequal. A zero time moved to another location is not `== time.Time{}`.
Report as: `<a> == <b>` at `<site>` compares location and monotonic data: equal instants differ, so `<dedupe / cache lookup / change detection>` misfires; fix: `a.Equal(b)`, `t.IsZero()`, and keys from `t.UTC().Round(0)` or `UnixNano()`.
