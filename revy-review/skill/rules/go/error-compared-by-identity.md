# error-compared-by-identity
langs: go

When: Errors compared with `==`, type-asserted or type-switched, checked with `os.IsNotExist`-style helpers, or an inspected error newly wrapped.
Ask: Can it arrive wrapped (`%w`, `errors.Join`, a repository layer)? Then `==` and `err.(*T)` miss it, and `os.IsNotExist`/`IsExist`/`IsPermission` never unwrap. For a diff that starts wrapping, search callers' `==` and type switches. Does an `errors.As` target match the returned type (`*E` vs `E` when both implement `error`)?
Not when: `err == io.EOF` on a `Read` result (the io contract returns EOF unwrapped).
Report as: `<check>` at `<site>` misses `<sentinel / type>` once `<site>` wraps it: the `<not-found / retry>` branch is skipped and `<consequence>`; fix: `errors.Is(err, <sentinel>)`, `errors.Is(err, fs.ErrNotExist)`, or `errors.As` with the returned type.
