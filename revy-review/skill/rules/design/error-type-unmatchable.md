# error-type-unmatchable
langs: any

When: Library API returns errors its callers must tell apart as strings, a generic exception, `Box<dyn Error>` or `anyhow::Error`.
Ask: Do callers branch on the kind of failure (retry, 404 vs 500, fallback)? Search them for message matching (`str(e)`, `err.Error() ==`, `contains`) and downcasts. Is it a library or crate boundary, where callers cannot see the concrete type?
Not when: application code at the top (`main`, a request handler) that only logs or reports the error.
Report as: callers of `<fn>` can tell `<kinds>` apart only by message text or downcasting, so a reworded message or a new cause breaks them silently; fix: a typed error (a `thiserror` enum, exception subclasses, Go sentinel or typed errors matched with `errors.Is`/`As`).
