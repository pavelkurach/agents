# same-fact-stored-twice
langs: any

When: Field, column or cache added that copies or derives from another stored value: a count beside a list, a status beside timestamps, a copied name.
Ask: Search every writer of the source value: does each also update the copy, in the same transaction? Could the value be computed on read, and does a measured read cost justify the copy?
Not when: one owner function writes both, and every writer goes through it.
Report as: `<copy>` duplicates `<source>`, and `<writer>` updates only `<source>`, so they drift and `<reader>` shows `<stale value>`; fix: derive it on read, or route every write through one owner that updates both.
