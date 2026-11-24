# body-change-unchanged-callers
langs: any

When: The diff changes a function's behaviour but not its signature: errors raised, return or empty case, ordering, units, defaults, or a side effect moved.
Ask: Find every caller: incoming call hierarchy and references, callers through the interface or base method it implements or overrides, and a text search for string-dispatched or cross-language uses (task names, RPC handlers, exports, callbacks). Read each caller the diff leaves untouched: does it catch the old error, branch on the old return or empty case, or rely on the side effect happening there?
Report as: `<fn>` now `<new behaviour>`; unchanged `<caller>` assumes `<old>` and will `<consequence>`; fix: update the callers, or keep the contract and add a new function.
