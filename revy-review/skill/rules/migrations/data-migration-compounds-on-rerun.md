# data-migration-compounds-on-rerun
langs: any

When: Data migration sets a value from its own current value (`x = x * 100`, a concatenation, an increment, a unit or encoding conversion), or inserts seed or reference rows.
Ask: Applied twice, does the data change again? A rerun comes from a non-atomic migration that failed after the step and was not recorded, a reverse-and-reapply, the same fix also run as a script or shipped in a second migration, or a rebuilt environment. Does the `UPDATE`'s `WHERE` select only rows still in the old state, through a marker or unit column set in the same statement? Do inserts conflict on a business key (`ON CONFLICT DO NOTHING`/`DO UPDATE`, `get_or_create`, `upsert`)? MySQL `INSERT IGNORE` also turns truncation and invalid-value errors into warnings.
Not when: an absolute assignment (`SET status = 'active' WHERE …`) that converges.
Report as: `<migration>` derives `<column>` from its current value with no old-state predicate: a second run `<multiplies / appends / duplicates>` it again and corrupts `<rows>` silently; fix: select only old-state rows (`WHERE unit = 'dollars'`, setting the marker in the same `UPDATE`), write into a new column, or upsert on the business key.
