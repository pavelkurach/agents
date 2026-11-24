# handler-missing-authz
langs: any

When: Handler, resolver, RPC method, admin action or job trigger added or changed that reads or changes a protected resource.
Ask: Is this action authorized on this specific object (owner or tenant match, role), beyond "is logged in"? Do identity, tenant and role come from the verified session or token, rather than the body, query, path or a client header? Do sibling routes on the same resource (list, export, bulk, nested GraphQL field) enforce it the same way — read how they do it?
Not when: the resource is public by design · a router-level policy visibly covers this action on this object.
Report as: `<handler>` lets `<any authenticated caller>` `<act>` on `<another user's or tenant's resource>`; fix: derive identity server-side, enforce `<policy>` at the boundary, and scope queries by tenant or owner.
