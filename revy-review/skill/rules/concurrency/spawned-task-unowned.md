# spawned-task-unowned
langs: any

When: Task, thread or goroutine spawned (`create_task`, `tokio::spawn`, `thread::spawn`, `go`, an unchained Promise) with no one awaiting its handle.
Ask: Who sees its failure and result, and who cancels or joins it at shutdown? In Python, is a strong reference kept — the loop holds tasks weakly, so an unreferenced task can be collected mid-run? Does dropping the handle detach the task (`JoinHandle`) or abort it (`JoinSet`)?
Not when: documented best-effort work that logs its own errors · framework-managed jobs (Celery, `BackgroundTasks`).
Report as: task at `<site>` has no owner: its failures are silent and it `<outlives its scope / is never cancelled / can vanish mid-run>`; fix: spawn into a group the parent awaits (`TaskGroup`, `JoinSet`, `errgroup`).
