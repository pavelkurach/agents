# iteration-error-unchecked
langs: go

When: Loop over `rows.Next()`, `scanner.Scan()`, an `iter.Seq2[T, error]` or a paging cursor added.
Ask: After the loop, is `rows.Err()` or `scanner.Err()` checked? These loops stop the same way on failure and on exhaustion, so a dropped connection or a line over 64 KiB (`bufio.MaxScanTokenSize`) ends them and the partial result looks complete. For `Seq2`, is `err` checked on each iteration? Are `rows` closed on early exit?
Report as: loop over `<rows / scanner>` at `<site>` never checks `Err()`: `<an I/O failure / a long line>` silently truncates `<result>`; fix: return `Err()` after the loop, and size `scanner.Buffer` for long lines.
