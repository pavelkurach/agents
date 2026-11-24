# gate-on-effective-setting
langs: any

When: Branch on a setting (flag, limit, policy) that another level can override: tenant over global, child over parent, request over default.
Ask: Is there a resolver for the effective value? Search for level-merging helpers and how sibling code makes the same decision.
Not when: the decision is about that level's own value (editing parent settings).
Report as: `<condition>` reads raw `<level>.<field>`: entities that override it take the wrong branch; fix: use `<resolver>`, or add one and route every such decision through it.
