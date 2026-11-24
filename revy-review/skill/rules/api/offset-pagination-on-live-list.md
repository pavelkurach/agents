# offset-pagination-on-live-list
langs: any

When: `offset`/`page` pagination or a total count added or changed on a list endpoint, export or batch loop.
Ask: Is the collection written while it is paged? Inserts and deletes shift offsets, so items are skipped or repeated — and a batch loop that updates or deletes the rows it pages through by offset skips half of them. How deep can pages go (the database reads and discards every skipped row), and is the total a `COUNT(*)` over a large table per request? Is the sort key unique, so ties keep their order between pages?
Not when: a small or rarely written collection · bounded admin tooling.
Report as: `<endpoint>` pages `<collection>` by offset while it changes: `<clients miss or duplicate items / the loop skips rows>` and deep pages scan `<n>` rows; fix: a keyset cursor on a unique sort key (`WHERE (k, id) > (:k, :id)`), and drop or cap the total.
