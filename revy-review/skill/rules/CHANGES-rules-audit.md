# Rules audit: changes

Bar: important and non-obvious to an Opus-class reviewer already running `reviewer.md` (impact tracing, the
edge-case matrix, the sibling-code comparison). Rule files only, `recipes/` excluded: 129 before, 132 after.

Across all kept rules: every `When` drops the leading "The diff …" and opens on the construct the reviewer sees, so
the index line starts with its leading word. Asks, Not-whens and Reports were tightened in place, with facts kept.

## Deleted (7)

| id | reason |
|---|---|
| design/bypasses-established-way | restates `reviewer.md` ("is this the way the codebase already does the job? Compare with the sibling code and existing helpers"); duplicate helpers are `recipes/added-symbol-duplicates-existing` |
| design/money-as-bare-number | textbook (float money, mixed currencies) |
| python/assert-as-runtime-check | textbook; linters flag it (`S101`/bandit `B101`) |
| python/default-evaluated-once | textbook; linters flag it (`B006`, `B008`, `RUF009`, `RUF012`) |
| rust/arc-mutex-single-owner | design taste; its real hazards (guard across `.await`, contention) are `concurrency/lock-held-across-await-or-io` |
| typescript/assertion-hides-absence | reviewers flag `!` and `as` unprompted (same reason `design/type-checker-bypassed` went); the unchecked-predicate fact moved to `typescript/external-data-typed-without-parsing` |
| typescript/unbound-method-callback | textbook; `@typescript-eslint/unbound-method` |

## Merged (3)

- `go/http-response-body-not-closed` → `resources/release-not-on-every-path` (the close-on-non-2xx and drain facts).
- `go/defer-in-loop` → `resources/release-not-on-every-path` (defer runs at function return; deferred `Unlock` deadlocks).
- `testing/untested-branch-matrix` → `testing/test-survives-mutation`: its coverage half restates `reviewer.md`'s Tests
  bullet; the "skip a branch" mutant and the branch dimensions (principal kind, variant, mode) moved into the mutation Ask.

## Renamed or refocused (4)

- `api/surface-wider-than-callers` → `api/whole-model-serialized`: kept only the whole-model serialization half (future
  columns leak); the exported-symbol half was taste.
- `design/unmeasured-optimization` → `design/fast-path-diverges`: from "add a benchmark" to the defect: two
  implementations of one computation that disagree on edge inputs, with no differential test.
- `rust/future-self-wake-spin` → `rust/poll-waker-contract`: adds the lost wakeup (waker registered after the
  readiness check, stale waker) beside the spin.
- `security/input-path-skips-boundary-parse` → `security/mass-assignment-from-payload`: the When was every entry
  point; now it fires on payloads spread onto entities, the case the rule was kept for.

## Notable widenings

- `concurrency/lock-held-across-await-or-io`: open DB transactions (row locks, a pooled connection) across I/O.
- `concurrency/check-then-act-race`: `FOR UPDATE` locks nothing for a row that does not exist yet.
- `api/breaking-contract-change`: strict consumers (`deny_unknown_fields`, closed enums) break on any new field.
- `api/offset-pagination-on-live-list`: batch loops that update the rows they page by offset skip half.
- `data/query-without-index`: Postgres does not index foreign-key columns.
- `data/stored-document-shape-changed`: messages already in queues and retry topics.
- `resources/unbounded-load-into-memory`: size limits checked before decompression.
- `design/dangerous-path-is-default`: permissive default arguments (`force=True`, an empty allowlist meaning all).

## Added (13)

| id | rationale |
|---|---|
| concurrency/process-local-state-assumed-global | an in-memory lock, rate limit, dedupe set or scheduler is per worker; with N copies it excludes nothing or runs N times |
| concurrency/lease-expires-mid-work | a lock TTL or visibility timeout lapses during slow work and a second worker runs the same job; needs fencing |
| concurrency/connection-shared-across-fork | a pool built before `--preload` or prefork fork is shared by children: crossed responses, inherited held locks |
| resilience/message-acked-before-processed | Celery acks before running by default, Kafka auto-commit commits regardless; a crash drops the message |
| resources/pooled-client-built-per-call | `create_engine`, `httpx.Client`, `boto3.client` per request: no reuse, leaked pools, port exhaustion |
| resources/bounded-pool-nested-acquire | holding a pool slot while waiting for another from the same pool deadlocks at pool-size concurrency |
| data/unordered-iteration-reaches-output | set, hash-map or un-`ORDER BY` order reaching keys, hashes or "first" picks differs across processes |
| data/soft-delete-leaks-into-queries | joins, counts and raw SQL miss the flag; unique constraints still count deleted rows |
| data/migration-imports-live-models | Django and Alembic migrations importing current models break once the models move on |
| security/backtracking-regex-on-input | nested quantifiers on input under a backtracking engine: one request pins a CPU (ReDoS) |
| security/token-claims-unchecked | a verified signature with no `aud`/`iss`/alg pinning accepts tokens minted for another service |
| rust/guard-lives-through-scrutinee | a guard temporary in a `match` or `while let` scrutinee lives through the arms: re-lock deadlock |
| python/orm-object-outlives-session | `expire_on_commit` and lazy relations raise `DetachedInstanceError` or `MissingGreenlet` after the session |

## Unsure

- `logic/edge-of-range-change`: kept against the audit's edge-case-matrix overlap, because EVIDENCE.md shows it as the
  strongest restored rule (6 useful findings only via this tag). Its Ask now targets the changed comparison and
  callers relying on the old edge.
