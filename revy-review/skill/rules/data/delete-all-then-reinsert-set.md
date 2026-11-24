# delete-all-then-reinsert-set
langs: any

When: Child or association rows synced by deleting all of them and inserting the new set.
Ask: Do other rows, audit logs, triggers or caches reference the child rows' ids or creation times? Do the delete and insert share one transaction, and what do two concurrent syncs produce? Does `ON DELETE CASCADE` remove more than the set?
Not when: no one references the rows' identity and the sync runs in one transaction.
Report as: `<sync>` deletes and recreates `<rows>`: `<references break / history churns / concurrent syncs duplicate or deadlock>`; fix: diff the sets, delete only stale rows, and upsert the rest on the business key.
