# privileged-action-unaudited
langs: any

When: Admin, support, impersonation, permission-grant or bulk-export action added or changed, or code that writes or deletes audit records.
Ask: Does the action write an audit record with actor, target, action and a ticket or reason — search how comparable actions are audited? Does it go to the append-only audit sink the codebase uses, rather than a table or file this same service can update or delete? Is it written on every path, failure and bulk variants included?
Report as: `<action>` leaves no `<tamper-resistant>` record of `<actor / target / reason>`: abuse by an insider or a stolen account cannot be detected or reconstructed; fix: emit to the existing audit sink with structured actor, target and justification.
