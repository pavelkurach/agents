# fault-path-untested
langs: any

When: New code protecting money, quotas or safety that relies on exactly-once or ordered delivery, transaction isolation, or a durable write acknowledgement.
Ask: Does a test deliver the same message twice or out of order, run the competing transactions concurrently, or fail between the side effect and the commit or ack — and then assert the invariant? Does that test fail with the protection (dedup key, lock, constraint) removed?
Report as: `<invariant>` relies on `<guarantee>` that no test breaks: a `<redelivery / crash / concurrent write>` that causes `<double charge / lost update>` would ship unnoticed; fix: a test that injects `<fault>` and asserts `<invariant>`.
