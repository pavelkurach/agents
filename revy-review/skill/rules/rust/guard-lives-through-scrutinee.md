# guard-lives-through-scrutinee
langs: rust

When: Lock guard or `RefCell` borrow created as a temporary in a `match` or `while let` scrutinee, or in an `if let` scrutinee before edition 2024.
Ask: Temporaries in a `match` scrutinee live until the whole `match` ends, so `match m.lock().unwrap().get(k) { … }` holds the lock through every arm; `while let Some(x) = q.lock().unwrap().pop()` holds it through the loop body; `if let` held it through `else` before edition 2024 (read `edition` in `Cargo.toml`). Does an arm or the body lock the same mutex again (deadlock), call `borrow_mut` on the same `RefCell` (panic), `.await` (guard across await), or do slow work?
Not when: the guard is bound with `let` and dropped or scoped before the branch.
Report as: guard from `<expr>` lives to the end of the `<match / while let>` at `<site>`: `<the arm re-locks and deadlocks / borrow_mut panics / contenders wait through the body>`; fix: bind the value first (`let v = m.lock().unwrap().get(k).cloned();`) and branch on `v`.
