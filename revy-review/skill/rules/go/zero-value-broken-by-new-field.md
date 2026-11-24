# zero-value-broken-by-new-field
langs: go

When: Field added to an exported or widely built struct: a map, channel, pointer or func, or a setting whose 0 means something.
Ask: Is the type built as `T{}`, `var t T`, `new(T)`, an embedding or a partial literal that skips the constructor? Search composite literals. A nil map panics on write, a nil channel blocks forever, a nil func panics on call, and a zero `Timeout`, `Limit` or `Retries` silently means none or unlimited. Was the zero value documented as ready to use?
Report as: new field `<T>.<f>` is unusable at its zero value: `<literals at sites>` that skip `New<T>` now `<panic on write / block / run unbounded>`; fix: lazy init in methods, a zero value that means the safe default, or an unexported field that forces the constructor.
