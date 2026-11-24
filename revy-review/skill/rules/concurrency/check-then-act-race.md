# check-then-act-race
langs: any

When: Shared state (row, file, cache key, remote resource) read, branched on, then written in a separate step.
Ask: Can another worker change it between the read and the write? Search other writers of the same table, key or path. A transaction alone closes nothing: a plain `SELECT` takes no lock under READ COMMITTED or REPEATABLE READ, and `SELECT … FOR UPDATE` locks nothing when the row does not exist yet, so insert-if-absent needs a unique constraint. Is there a row lock in the same transaction, or a constraint that rejects the loser?
Not when: the check lives inside the write — `UPDATE … WHERE <check>` with a rowcount test, `INSERT … ON CONFLICT`, compare-and-set, `O_EXCL`.
Report as: `<check>` then `<write>` on `<state>` races: two `<actors>` both pass the check and `<double-apply / insert duplicates>`; fix: fold the check into the write, a unique constraint, or `SELECT … FOR UPDATE` on an existing row in the same transaction.
