# shared-object-mutated-unsynchronized
langs: any

When: Write to global, module or static state, a class attribute, or a field of an object outliving the request (singleton, app-scoped client, cache).
Ask: Find its construction and lifetime (startup, DI scope, cached factory): who reaches it concurrently? Does it now hold per-request data (user, tenant, token)? Is the write synchronized, and can another task run at an `await` between the write and the read?
Not when: set once at startup before concurrency starts · an immutable value replaced by one reference swap.
Report as: shared `<object>`.`<field>` is written per request: requests read each other's `<data>` (a cross-tenant leak) or lose updates; fix: pass request data explicitly or through `contextvars` or the request context.
