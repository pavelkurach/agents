# edge-of-range-change
langs: any

When: Comparison (`<`/`<=`), range or slice bound, loop limit, pagination offset, `±1` index arithmetic, or empty or maximum input handling changed.
Ask: Evaluate the old and the new expression at empty, one, exactly the bound and the maximum: which caller can send each value? Where the edge behaviour changed, who relied on the old one — search callers, stored data and tests at that value?
Report as: at `<edge>`, `<expr>` now `<includes / excludes / indexes past>` `<element>`: `<consequence>`; fix: `<bound>`, plus a test at that value.
