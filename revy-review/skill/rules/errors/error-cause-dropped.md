# error-cause-dropped
langs: any

When: Caught error wrapped, translated or logged.
Ask: Do the new error and the log keep the cause and traceback? They are lost by `raise … from None`, `map_err(|_| …)`, a `thiserror` variant without `#[source]` or `#[from]`, Go `%v` instead of `%w` (search upstream `errors.Is`/`As`), `new Error(msg)` without `{ cause }`, `logger.error` instead of `logger.exception` inside `except`, `anyhow` crossing a boundary without `.context()`.
Not when: Python `raise New()` inside `except` without `from` (the implicit context still prints).
Report as: `<site>` drops the cause of `<error>`: `<the traceback is lost / errors.Is fails / no context>`; fix: chain it (`from e`, `%w`, `#[source]`, `.context()`, `cause`) or log the traceback.
