# rename-or-drop-while-old-code-runs
langs: any

When: Migration renames or drops a column, table or unique constraint, or changes a column type incompatibly, in the release that changes the code.
Ask: Do old instances keep running during the rollout or after a rollback? Most ORMs select every mapped column, so old code fails on every query of the table, beyond those that use the column. Does any code depend on the dropped unique constraint (`ON CONFLICT (<cols>)`, duplicate handling)? Do other services, ETL jobs or replicas read it?
Not when: the table has no production traffic yet · a previous release already stopped reading and writing the old shape.
Report as: `<migration>` `<renames / drops>` `<object>` while `<old code / consumer>` still uses it: queries fail during deploy and rollback is impossible; fix: expand–contract (add new, dual-write, backfill, switch reads, drop in a later release).
