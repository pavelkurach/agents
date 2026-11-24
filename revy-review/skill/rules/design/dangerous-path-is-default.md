# dangerous-path-is-default
langs: any

When: API or parameter added whose shortest form is the risky one: unchecked, panicking, unescaped, unbounded, destructive, or unscoped to the caller's tenant.
Ask: Is there a safe sibling, and which form do callers in the diff and the codebase use? Does a default argument pick the permissive side (`force=True`, `dry_run=False`, `allow_all=True`, an empty allowlist meaning everything)? Is the risky form visibly named at the call site (`_unchecked`, `unsafe`, `raw_`, `unscoped_`)?
Not when: an internal helper whose only caller already guarantees the check.
Report as: the plain call `<api>` `<skips check>`, so callers `<panic / inject / destroy data / cross tenants>` without choosing to; fix: give the checked, narrow form the plain name and defaults, and move the risky one behind an explicit `<_unchecked / raw / force>` variant.
