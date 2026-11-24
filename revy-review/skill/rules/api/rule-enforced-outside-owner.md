# rule-enforced-outside-owner
langs: any

When: Validation, invariant or workaround added in a caller, gateway or adapter for data or behaviour that another module or service owns.
Ask: Which other writers reach the owner directly — search the owner's API and every writer of its table or topic — and do they bypass the new rule? Does the workaround compensate for a bug in the owner, and will it break or double-apply once the owner is fixed?
Not when: an input-shape check at this layer's own boundary · the owner documents that callers enforce it.
Report as: `<rule>` lives in `<caller>` while `<owner>` owns `<data>`: `<other path>` bypasses it and `<violation>`; fix: enforce it in `<owner>`, or fix the owner and drop the workaround.
