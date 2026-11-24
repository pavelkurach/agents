# unawaited-async-call
langs: python, typescript, rust

When: Coroutine-, Promise- or Future-returning function called without `await`, or an existing function made async.
Ask: For a function made async, search every caller, unchanged ones included. Is each result awaited or handed to something that awaits it? Check the cases linters miss: a truth test (a coroutine or Promise is always truthy), attribute access on the result, `let _ = fut;`.
Report as: `<call>` is never awaited: `<the body never runs / the condition is always true / the error goes unseen>`; fix: await it, making the caller async.
