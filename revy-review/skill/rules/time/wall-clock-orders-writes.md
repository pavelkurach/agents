# wall-clock-orders-writes
langs: any

When: Conflicting writes resolved or events ordered by wall-clock time: an `updated_at` comparison, last-write-wins, a newest-wins merge.
Ask: Can competing writes come from different hosts, or land within the clock's resolution? Who sets the timestamp — app clocks skew, and Postgres `now()` is the transaction start, so a later commit can carry an earlier time.
Report as: `<merge>` picks the winner by `<field>`: skew lets an older write overwrite a newer one silently; fix: a version column (`WHERE version = :v`), a sequence, or a hybrid logical clock.
