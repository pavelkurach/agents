# promise-combinator-failure-semantics
langs: typescript

When: Concurrent work run through `Promise.all`, `allSettled`, `race` or `any`, or switched between them.
Ask: With `all`, the first rejection fails the call while the other operations keep running and their side effects land: does the caller assume nothing happened? With `allSettled`, is each `status` read (it never rejects)? Does a `race` against a timeout stop the loser (only with an `AbortSignal`)? Is the input bounded, or does `items.map(call)` start thousands of calls at once?
Report as: `<combinator>` at `<site>` `<reports failure while other writes complete / hides rejections / leaves the loser running / starts N calls at once>`: `<partial state / rate limiting / exhaustion>`; fix: `allSettled` with per-item handling, abort signals for losers, a concurrency limit.
