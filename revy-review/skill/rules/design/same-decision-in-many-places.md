# same-decision-in-many-places
langs: any

When: Branch on a kind or status, a format or wire detail, a limit literal, or a field default added or edited that other modules or layers also encode.
Ask: Grep the predicate, literal, format or default across the repository; for a default, read migrations (`server_default`), models, request schemas, settings and constructors. Did the diff change every copy the same way, and does one now disagree? Which copy applies on each path (API create, raw SQL insert, import job, test factory), and how many sites must the next change touch?
Not when: the copies sit behind separately versioned contracts that must be free to diverge (a frozen API version, an old migration) · the layers derive from one constant.
Report as: `<decision / default>` is encoded in `<n>` places and `<site>` `<now disagrees / will be missed next time>`: `<rows get different values depending on the path that writes them / consequence>`; fix: give it one owner — a method on the type, a dispatch table, a named constant — that every site and layer references.
