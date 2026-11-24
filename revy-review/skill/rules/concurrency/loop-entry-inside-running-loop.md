# loop-entry-inside-running-loop
langs: python, rust

When: `asyncio.run`, `run_until_complete`, `Runtime::block_on` or `futures::executor::block_on` added, or new callers given to a function containing one.
Ask: Search callers transitively: does any run inside a running loop — an async handler, an async test fixture, a notebook, a sync callback invoked from async code?
Not when: tokio `block_in_place` on a multi-thread runtime wraps it.
Report as: `<entry>` in `<fn>` runs inside a loop via `<caller>`: `RuntimeError` (asyncio) or a panic or deadlock (tokio); fix: make `<fn>` async and await it.
