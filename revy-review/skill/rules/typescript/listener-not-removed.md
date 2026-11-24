# listener-not-removed
langs: typescript

When: `addEventListener`/`on`/`subscribe`/`setInterval` in code that runs repeatedly (component, effect, handler, reconnect), or a change to its removal.
Ask: Is it removed when its owner goes away, with the same function reference and the same `capture` flag? A fresh arrow or `.bind(this)` at removal is a different function, so `removeEventListener` silently does nothing. Does each effect return a cleanup? Is Node's `MaxListenersExceededWarning` (past 10 per event) being silenced instead?
Report as: listener at `<site>` is added per `<render / request / reconnect>` and never removed: handlers pile up, run N times and pin `<closure state>`; fix: keep the reference and remove it in cleanup, or pass `{ signal }` from an `AbortController` aborted on teardown.
