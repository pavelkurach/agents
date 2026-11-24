# mutation-not-retry-safe
langs: any

When: Side-effecting create, charge, send or publish endpoint, or a queue or webhook handler, added or changed.
Ask: What retries it — client retries after a timeout, gateway retries, at-least-once broker redelivery? Applied twice, does the effect happen twice? For an idempotency key: does the client supply it, stable across retries (a server-minted key never matches the retry); is it scoped per caller, stored with the result under a unique constraint; does a replay return that stored result rather than an error?
Not when: an overwrite by a known key (`PUT` or `DELETE` by id) · a unique constraint on the business key that turns the duplicate into a lookup of the existing row.
Report as: `<operation>` repeats its effect under `<retry source>`: one lost response `<double-charges / sends twice / creates a duplicate>`; fix: a client idempotency key stored with the result under a unique constraint, or an upsert.
