# soft-delete-leaks-into-queries
langs: any

When: Soft delete (`deleted_at`, `is_deleted`, `archived`) added, or a query, join, count or unique constraint added on a soft-deleted table.
Ask: Does every read path exclude deleted rows — raw SQL, joins from other tables, related-object access, counts, exports, search indexing — or only the default manager or scope? Search the table name across queries. Does a unique constraint still count deleted rows, so re-creating the same name or email fails (it needs a partial index `WHERE deleted_at IS NULL`)? Do foreign keys and cascades still treat deleted parents as live?
Report as: `<query / constraint>` on `<table>` ignores the soft-delete flag: `<deleted rows reappear / counts include them / re-creation fails on the unique key>`; fix: a partial unique index over live rows, and one scoped query path (default scope, view or repository method) that every reader uses.
