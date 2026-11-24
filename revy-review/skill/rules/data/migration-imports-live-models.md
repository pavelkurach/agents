# migration-imports-live-models
langs: python

When: Data migration or backfill imports application models, services or constants instead of using the schema as of that migration.
Ask: Django: does it import models directly instead of `apps.get_model(...)`? Alembic: does it import ORM models or app code instead of defining the table inline with `sa.table(...)`? A migration runs against the schema of its own point in history, but imported code tracks the latest models: once a later change adds, renames or drops a field, this migration breaks on fresh databases, in test setup and in every environment that is behind. Does it trigger business logic (signals, `save()` overrides, services) that should not run inside a migration?
Report as: `<migration>` imports `<model / service>` from current code: once `<model>` changes, the migration `<fails / writes the wrong columns>` on fresh databases and lagging environments; fix: the historical model (`apps.get_model`) or an inline table definition, with the needed logic copied into the migration.
