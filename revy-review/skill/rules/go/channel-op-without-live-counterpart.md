# channel-op-without-live-counterpart
langs: go

When: Goroutine sending or receiving on a channel, a `select` with a timeout or `ctx.Done()` branch, or a channel `close` added.
Ask: Is each send, receive and `range ch` matched by a live counterpart on every path? A receiver that returns early (timeout, ctx, first error) strands a sender on an unbuffered channel; a `range` needs its producer to close. Who closes: a close by a receiver, or by one of several senders, makes another sender panic. A nil channel blocks forever.
Not when: the sender also selects on `ctx.Done()` · the buffer holds every send.
Report as: goroutine at `<site>` blocks forever on `<send / receive>` when `<caller returns early>`: one goroutine leaks per `<request>` (or `<close>` by a non-owner panics a sender); fix: a buffer sized to the sends, `select` on `ctx.Done()` in the sender, or one owner that closes after all senders finish.
Example:
  bad:  ch := make(chan res); go func() { ch <- call() }(); select { case r := <-ch: …; case <-ctx.Done(): return }
  good: ch := make(chan res, 1)
