# doc-overclaims-guarantee
langs: any

When: Docstring, README, comment or error message added or changed that states a guarantee: never, always, exactly, guaranteed, at most.
Ask: Read the code the statement describes: does it deliver the guarantee on every path, or only approximately — through a cache, a sampled or heuristic signal, a best-effort retry, a race window, a lenient parse?
Report as: `<doc>` promises `<guarantee>`, but `<path>` only `<what it actually does>`, so callers relying on it `<consequence>`; fix: state the real bound, or make the code meet the promise.
