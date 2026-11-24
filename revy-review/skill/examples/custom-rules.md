# duplicated-logic
langs: any

When: always
Ask: For every function, branch and block the diff adds or changes: does equivalent logic already exist? Search with code intelligence (same callees, similar signatures) and text search (name stems, literals, regexes, error messages, SQL), across the whole repository and between the diff's own new blocks. Read each candidate before deciding.
Not when: the copies are meant to diverge (different owners or release cycles) and say so in a comment, or the shared part is under ~3 lines.
Report as: `<new path:line>` duplicates `<existing path:line>` (<what is the same>); fix: reuse the existing code, or extract one shared function and call it from both.
