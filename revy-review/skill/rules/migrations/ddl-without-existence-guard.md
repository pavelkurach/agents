# ddl-without-existence-guard
langs: any

When: SQL file or script runs `CREATE`/`DROP`/`ALTER` outside a framework's applied-version tracking, or as a Flyway `R__` or Liquibase `runOnChange`/`runAlways` changeset.
Ask: What runs it again? Container init scripts and startup hooks run on every boot. Flyway repeatable migrations rerun whenever their checksum changes, and `runAlways` changesets run on every update. Does each statement converge on a second run? Guard with `IF [NOT] EXISTS` or `CREATE OR REPLACE`. Postgres has no `IF NOT EXISTS` for `ADD CONSTRAINT` or `CREATE TYPE`, so those need a catalog check in a `DO` block. MySQL has no `IF [NOT] EXISTS` on columns or indexes (MariaDB has), so it needs an `information_schema` check. `IF NOT EXISTS` matches only the name: an object whose definition changed (index columns, column type) is silently left in its old form.
Not when: a versioned migration (`V__`, Django, Alembic, Rails, sqlx, diesel, Prisma) that applies atomically and is recorded; for one that is not, see `migrations/migration-half-applied-without-transaction`.
Report as: `<statement>` in `<script>` has no existence guard, and `<runner>` reruns it: the second run `<fails on the existing object / keeps the stale definition>`; fix: guard each statement (`IF NOT EXISTS`, a catalog check, `CREATE OR REPLACE`), or move it into a versioned migration.
