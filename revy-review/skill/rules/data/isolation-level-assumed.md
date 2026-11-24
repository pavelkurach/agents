# isolation-level-assumed
langs: any

When: Transaction isolation level set or changed, or relied on to protect an invariant across concurrent transactions.
Ask: Which anomaly must it prevent, on which engine? Postgres `REPEATABLE READ` and Oracle `SERIALIZABLE` are snapshot isolation, so write skew passes; MySQL `REPEATABLE READ` locking reads and updates see the latest rows, not the snapshot. Under `SERIALIZABLE` or `REPEATABLE READ`, is a serialization failure (SQLSTATE `40001`) retried, or does it surface to the caller as an error?
Not when: no invariant spans concurrent transactions.
Report as: `<transaction>` relies on `<level>` to prevent `<anomaly>`, which `<engine>` allows: `<invariant>` breaks only under contention; fix: `SELECT … FOR UPDATE` on the rows the invariant reads, a constraint, or true `SERIALIZABLE` with a retry loop.
