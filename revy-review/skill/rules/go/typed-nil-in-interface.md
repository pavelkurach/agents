# typed-nil-in-interface
langs: go

When: Pointer, map, slice or func variable returned or assigned as an `error` or other interface, or an interface value compared with `nil`.
Ask: Can the concrete value be nil there — a `var e *MyErr` returned on the success path, a constructor returning a nil `*impl` as an interface? An interface holding a typed nil is `!= nil`, so `err != nil` fires and method calls get a nil receiver. Read every return path.
Not when: every no-value path returns a literal `nil`.
Report as: `<fn>` returns a nil `<*T>` as `<interface>`: it compares `!= nil`, so callers `<treat success as failure / call methods on nil>`; fix: return a literal `nil` on that path, and keep concrete error types out of local result variables.
Example:
  bad:  var e *ValidationErr; if bad { e = &ValidationErr{} }; return e
  good: if bad { return &ValidationErr{} }; return nil
