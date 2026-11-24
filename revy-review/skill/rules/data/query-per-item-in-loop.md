# query-per-item-in-loop
langs: any

When: DB query or remote call added inside a loop, comprehension, serializer or resolver, or into a function that callers may invoke per item.
Ask: Search the changed function's callers: does any call it in a loop? Does attribute access fire a query per item — lazy ORM relations, nested serializer fields, GraphQL field resolvers? Is there a bulk form: `IN` or `= ANY($1)`, `selectinload`/`joinedload`, `select_related`/`prefetch_related`, a DataLoader, a batch API?
Not when: the item count has a small fixed bound.
Report as: `<loop>` issues one `<query / call>` per `<item>`: N round trips per request, so latency grows with the data and the dependency takes N× load; fix: one batched query, or an eager load of `<relation>`.
