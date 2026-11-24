# not-null-column-on-live-table
langs: any

When: Migration adds a `NOT NULL` column, or sets `NOT NULL` on an existing column, of a table that already has rows.
Ask: Is there a server-side default, or a nullable → backfill → `NOT NULL` sequence? Postgres rejects `ADD COLUMN … NOT NULL` without a default on a non-empty table, and runs `SET NOT NULL` as a full scan under an exclusive lock (skipped since PG 12 when a validated `CHECK (col IS NOT NULL)` exists); MySQL fills the type's implicit default (`0`, `''`). Will old instances still running during the rollout insert without the column and fail? Does new code read it as non-null before the backfill finishes?
Not when: the table is created in the same migration · a constant server default covers old rows and old writers.
Report as: `<migration>` makes `<column>` `NOT NULL` without `<default / backfill>`: `<the migration fails / old writers' inserts fail / reads hit nulls>` during deploy; fix: add it nullable with a server default, backfill in batches, then set `NOT NULL` in a later migration.
