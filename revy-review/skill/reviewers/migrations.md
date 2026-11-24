rules: migrations, data

Review the schema and data migrations this change adds or alters, and the code they meet during the rollout: models and queries on the touched tables, backfill jobs, the deploy step that runs migrations, and old instances still serving traffic. Go deepest on three properties for every migration:
- Concurrent: it runs against live production traffic and row counts. Name each lock it takes, how long the lock is held, and what else waits behind it.
- Idempotent: a second application converges on the same schema and data.
- Rerunnable: after a failure at any step it can run again from where it stopped, and it neither corrupts data nor reports success on an incomplete result.

Walk each migration as the deploy runs it: old code on the new schema, new code on the old schema, a failure between two statements, a rerun. When the change adds or alters no migration, say so plainly in the ledger.
