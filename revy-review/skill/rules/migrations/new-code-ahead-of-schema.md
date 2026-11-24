# new-code-ahead-of-schema
langs: any

When: Code reads or writes a column, table or enum value that a migration in the same change adds, or stops writing a column the schema keeps as `NOT NULL`.
Ask: When do migrations run relative to the rollout? The options are a release phase, an init container, a Helm pre-upgrade hook, a post-deploy step or a manual run; find which one the repo uses. Once new instances serve traffic before the migration lands, or after it fails, queries on the new column fail. Code that stops writing a `NOT NULL` column with no server default fails every insert until that column becomes nullable or gets a default. Rails puts every cached column in its `INSERT`s, since `partial_inserts` is off by default from 7.0, until `ignored_columns` lists the column. Old code running on the new schema: `data/rename-or-drop-while-old-code-runs`, `data/not-null-column-on-live-table`.
Not when: the deploy blocks traffic to new code until migrations succeed, and old code is compatible with the new schema.
Report as: `<code>` `<reads / writes / stops writing>` `<column>` while the schema `<lacks it / still requires it>` during `<deploy step>`: `<queries / inserts>` fail until the migration runs; fix: make the schema change backward compatible first (nullable or defaulted, `ignored_columns`, `SeparateDatabaseAndState`), ship it, then ship the code that depends on it.
