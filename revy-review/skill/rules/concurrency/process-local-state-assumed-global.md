# process-local-state-assumed-global
langs: any

When: In-memory lock, cache, counter, rate limiter, dedupe set or scheduler added to a service that runs as several processes or replicas.
Ask: How many copies run — web workers (`gunicorn`/`uvicorn --workers`), replicas, Celery prefork children? Read the deploy config. Does correctness need one instance across all of them: mutual exclusion, a global rate limit, dedupe of a redelivered message, a job run once per period, an invalidation every reader sees? An in-process scheduler (APScheduler, `setInterval`, a `tokio` interval started at boot) fires once per copy.
Not when: per-process state by design — a memo of immutable data, a per-connection limit.
Report as: `<state>` lives per process, but `<invariant>` needs it global: with `<n>` copies the `<lock excludes nothing / limit is n× / job runs n times / caches serve stale data>`; fix: move it to the shared store (a DB lock or unique constraint, Redis, a single scheduler), or size and document it per process.
