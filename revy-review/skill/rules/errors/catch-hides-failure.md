# catch-hides-failure
langs: any

When: Error handler added, widened or changed: `except`, `catch`, `if err != nil`, `Err(_) =>`, `.ok()`, `let _ =`, `unwrap_or_default`.
Ask: What can reach it — would a bug (`TypeError`, `KeyError`, a null dereference) land here too? Does a bare `except:` or `except BaseException` swallow `CancelledError` or `KeyboardInterrupt`? Afterwards, can the caller tell the operation failed, and does it need to (to retry, roll back, report)?
Not when: it isolates one unit of a batch or a top-level task and records the failure · the fallback is correct domain behaviour (cache miss → recompute).
Report as: `<handler>` `<catches too much / discards>` `<errors>`: failures become a silent `<default>` and `<caller>` continues on wrong data; fix: catch only `<expected errors>` and propagate the rest.
