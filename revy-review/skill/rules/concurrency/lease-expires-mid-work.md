# lease-expires-mid-work
langs: any

When: Work guarded by a lease or TTL: a distributed lock with expiry, a queue visibility timeout, a job claim with a timeout, leader election.
Ask: Can the work outlast the lease — a slow dependency, a GC or event-loop pause, a large input — so the lease lapses and a second worker starts the same job while the first still runs? Is the lease renewed while working, and does the holder confirm it still holds the lease before each write? Do writes carry a fencing token that the store checks? Defaults to compare with: SQS visibility timeout 30 s, Celery's Redis and SQS broker `visibility_timeout` 1 h.
Not when: the work is idempotent and two concurrent runs are harmless.
Report as: `<work>` can outlive `<lease>` (`<ttl>`): a second worker takes `<job / lock>` while the first runs, and both `<write / charge / send>`; fix: renew the lease from a heartbeat, fence writes with a monotonically increasing token, or make the work idempotent.
