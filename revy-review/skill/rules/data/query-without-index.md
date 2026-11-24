# query-without-index
langs: any

When: Query filter, join, sort or lookup key added or changed (`WHERE`, `JOIN ON`, `ORDER BY`, ORM `filter`/`order_by`) on a table that grows.
Ask: Read the table's migrations or model: does an index lead with these columns? Does a composite index exist only in an order the query cannot use? Does a function or cast on the column (`lower(email)`, `created_at::date`) stop the index applying? Postgres does not index foreign-key columns automatically (InnoDB does), so joins and `ON DELETE CASCADE` from the parent scan the child table.
Not when: a small fixed-size table · a one-off script.
Report as: `<query>` filters `<table>` on `<columns>` with no usable index: a full scan per call, fast in dev and `<seconds>` at production row counts; fix: add `<index>` in the same change, built without blocking writes.
