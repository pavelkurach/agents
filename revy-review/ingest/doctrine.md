# Applying the test

The test itself is in `SKILL.md`. This file is what you and the critic consult while applying it: the verdicts, and
one rule on each side of the line.

## Verdicts

Record one per claim, with a one-line reason that names the evidence.

| verdict | when | reason names |
|---|---|---|
| keep | effective, concise and generic | the non-obvious fact the rule carries |
| merge `<id>` | the fact belongs on an existing rule's trigger | the rule, and what it gains (an Ask fact, a wider When, a Not when) |
| supports `<id>` | the claim only backs a fact an existing or drafted rule already carries; neither a new rule nor a cut | the rule, and the fact it backs |
| own | effective and concise, but true only of the owner's own systems (their postmortem, their services) | the system; it goes to `~/.revy-review/rules/` |
| cut: known | a strong reviewer running `reviewer.md` raises it unprompted: textbook, "be careful with X", "validate input", "add tests", "handle errors" | the default behaviour that already covers it |
| cut: linter | a check that runs by default in the repo's CI or editor flags it: the compiler, a default-on lint. An opt-in tool that usually does not run (zizmor, CodeQL for Actions, Docker build checks) leaves the rule standing | the check (`B006`, `clippy::…`, `@typescript-eslint/…`) and where it runs |
| cut: covered | an existing rule or `reviewer.md` already states it | the rule id or the `reviewer.md` line |
| cut: no trigger | nothing visible in a diff marks where it applies (architecture, process, team practice) | why no diff shows it |
| cut: unverifiable | the fact cannot be confirmed against docs, source code, a changelog or a reproduction | what was checked |
| cut: specific | true only of the source's codebase or vendor, and the owner does not run it | the source-only name |

## Effective: carries the fact the reviewer lacks

`migrations/ddl-waits-in-lock-queue` (excerpt):

```markdown
When: Migration runs any `ALTER`, `DROP`, `RENAME`, `TRUNCATE` or foreign-key add on an existing table, including changes that are metadata-only or instant.
Ask: Does the migration set a lock timeout before the DDL? Postgres `lock_timeout` defaults to 0 and MySQL `lock_wait_timeout` to one year. While the DDL waits for its lock, every later query on the table queues behind it, including plain reads, …
```

A strong reviewer sees an instant `ADD COLUMN`, knows it is metadata-only, and approves. The rule changes that
verdict with two defaults and one counter-intuitive consequence: the cheap statement queues every read behind a long
transaction, and the deploy becomes an outage. Each fact is checkable in the database docs.

## No-op: restates what the reviewer already does

The audit (`../skill/rules/CHANGES-rules-audit.md`) deleted `python/default-evaluated-once` as "textbook; linters
flag it". Its shape:

```markdown
When: Function with a mutable default argument (`def f(items=[])`).
Ask: Is the default shared across calls? Python evaluates defaults once, at definition.
```

Every strong reviewer flags `def f(items=[])` without the rule, and `B006` flags it before review. The rule spends
index space and moves no verdict: **cut: known**, **cut: linter**.

## Concise and generic, side by side

| fails | rewrite |
|---|---|
| When: "Code that deals with concurrency." (a principle; matches every diff) | `concurrency/process-local-state-assumed-global` — When: "In-memory lock, cache, counter, rate limiter, dedupe set or scheduler added to a service that runs as several processes or replicas." |
| When: "`orders-svc` calls `/v2/charge` without `X-Idem-Key`." (source-only names) | already `api/mutation-not-retry-safe`: **cut: covered**, or **own** when the owner runs `orders-svc` |
| Ask: a page on how locks work, then the question | Ask: the question, then only the facts that change its answer |
