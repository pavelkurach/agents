# non-atomic-multi-write
langs: any

When: Dependent writes that commit separately — DB plus message or API call, two transactions, `INCR` then `EXPIRE` — or a transaction split in two.
Ask: If the process dies after the first write, what state remains, and what does a retry or redelivery do with it? Is a message or job published before the commit that makes it true, so a worker reads the row before the commit or after a rollback?
Not when: an outbox, compensation step or idempotency key covers the gap · the commands run as one atomic unit (`MULTI/EXEC`, a script, `SET … EX`).
Report as: `<write 1>` commits before `<write 2>`: a crash between leaves `<state>` and a retry `<double-applies / never finishes>`; fix: one transaction, a transactional outbox, or idempotent steps keyed by `<id>`.
