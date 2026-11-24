# iterator-consumed-twice
langs: python

When: Generator, `map`/`filter`/`zip` result or file object passed to code that may iterate it twice, or a list return changed to an iterator.
Ask: Does any consumer iterate it twice, test membership with `in` and then loop, or check its truthiness (a generator is always truthy)? Search callers of a changed function for a second pass: it silently sees nothing.
Report as: `<iterator>` is used up by `<first use>`, so `<second use>` sees nothing and `<returns empty / miscounts>`; fix: `list()` it once, or return a `Sequence`.
