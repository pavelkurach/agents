# connection-shared-across-fork
langs: python

When: Connection, pool, client or lock created at import time or in a parent that later forks: `gunicorn --preload`, `multiprocessing` with fork, Celery prefork.
Ask: Is the object created before the fork and used in the children? Then the children share one socket and interleave protocol bytes (crossed or corrupted responses, SSL errors, "lost synchronization with server"), and a lock another thread held at fork time stays locked forever in the child. Does each child reset it — SQLAlchemy `engine.dispose(close=False)` after fork, `os.register_at_fork(after_in_child=…)`, gunicorn `post_fork`, Celery `worker_process_init` — or create it lazily on first use per process? `multiprocessing` forks by default on Linux before Python 3.14.
Not when: the start method is `spawn` or `forkserver` · the object is created after the fork.
Report as: `<object>` is created before the fork at `<site>` and shared by every child: `<responses cross between processes / a child deadlocks on the inherited lock>`; fix: create it per process after the fork, or dispose and reopen it in a post-fork hook.
