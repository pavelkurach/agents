# migration-half-applied-without-transaction
langs: any

When: Migration with several statements runs without a wrapping transaction: on MySQL, or with `atomic = False`, `disable_ddl_transaction!`, `autocommit_block()` or `executeInTransaction=false`.
Ask: MySQL commits implicitly before and after every DDL statement, so DDL and data steps in one migration apply one by one. If statement k fails, what is left applied, and what does the framework record? Django and Alembic record nothing, and their rerun starts again at statement 1, where the earlier DDL fails (`Duplicate column`) or a data step compounds (`migrations/data-migration-compounds-on-rerun`). Flyway marks the version failed, and stays blocked until a manual cleanup and `repair`; Prisma blocks `migrate deploy` (P3009) until `migrate resolve`. Can each step run again on a database where it already applied, and does a failure leave the schema in a state the running code accepts?
Not when: one statement per migration · every step guarded and convergent.
Report as: `<migration>` applies `<statements>` with no transaction: a failure at `<step>` leaves `<partial schema>` unrecorded, and the rerun `<fails on the applied step / repeats the data change>`; fix: one DDL statement per migration, or guard each step so it converges on rerun, with data steps in their own migration.
