# lock-held-across-await-or-io
langs: any

When: I/O, an `await`, a callback or long computation runs while a lock guard or an open DB transaction is live.
Ask: Where is the guard released — scope end, `drop`, or the end of the statement for temporaries? Every contender waits out the I/O. A sync mutex held across `await` blocks other tasks on that thread; in Rust a `std::sync` guard still in scope at an `.await` makes the future `!Send` even if unused after. An open transaction holds its row locks and a pooled connection across an HTTP call or `await`: how long can the call take, and how many requests make it at once?
Not when: an async lock deliberately serializes the awaited work (single-flight refresh).
Report as: `<lock / transaction>` held across `<io / await>` in `<fn>`: contenders `<stall / drain the pool>` for the call's duration `<and the executor can deadlock>`; fix: copy under the lock or commit first, release, then do the I/O.
