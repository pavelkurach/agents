# sentinel-as-default
langs: any

When: Explicit empty, null, zero or negative value mapped to a default id, path, scope, tenant or limit (`x or DEFAULT`, `x || d`, `limit <= 0` → unlimited).
Ask: Can a caller send it by mistake — an unset variable, a failed lookup, a Go zero value, `unwrap_or_default()`? Where does the default point: another resource, a wider scope, no limit? Does the boundary tell "omitted" from "empty"?
Not when: the default is narrower than any real value · display options.
Report as: explicit `<sentinel>` for `<param>` becomes `<default>`: bad input `<hits the wrong resource / widens scope>` instead of failing; fix: default only on omission, and reject the sentinel.
