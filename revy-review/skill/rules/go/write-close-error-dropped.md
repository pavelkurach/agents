# write-close-error-dropped
langs: go

When: Writes through a file or buffering writer (`bufio`, `gzip`, `zip`, `csv`, an upload stream) that end in `defer w.Close()` or an unchecked `Flush`.
Ask: Does the final `Flush` or `Close` write data — buffered bytes, a gzip or zip footer, an upload commit — whose error is discarded? `csv.Writer.Flush` reports failure only through `Error()`. Must the file survive a crash (`Sync`)?
Not when: read-only handles.
Report as: `<writer>` at `<site>` drops the `<Flush / Close>` error: a truncated or corrupt `<file / object>` is reported as written; fix: check `Flush` and `Close` on the success path and return the error, keeping the deferred close for error paths.
