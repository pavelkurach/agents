# retry-without-backoff
langs: any

When: Retry added or changed: a loop, `tenacity` or `backoff`, client retry config, a redelivery policy.
Ask: Is the delay growing and jittered, with an attempt or time cap, and are permanent errors (4xx, validation) excluded? Could a timed-out attempt have succeeded, making the retry a second write? Do other layers retry too — client, SDK, gateway, queue — so attempts multiply (3 × 3 × 3 = 27 per request)?
Report as: `<retry>` on `<op>` is `<immediate / unbounded / on every error / stacked on <layer>>`: it hammers the failing dependency `<and applies op twice>`; fix: capped jittered backoff on transient errors, one retrying layer, an idempotency key.
