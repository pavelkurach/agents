# orm-object-outlives-session
langs: python

When: ORM object returned, cached, handed to a thread or task, or read after its session closes or commits.
Ask: Does later code touch a lazy relationship or an expired attribute? SQLAlchemy expires every attribute on `commit()` by default (`expire_on_commit=True`), so the next read issues a query: `DetachedInstanceError` once the session is closed, `MissingGreenlet` under `AsyncSession`, which forbids implicit IO. A lazy relationship read after the request's session ends fails the same way. Is the object, or its session, used from another thread or task while this one still uses it (sessions are not thread- or task-safe)?
Not when: the attributes and relationships it reads are loaded eagerly (`selectinload`, `expire_on_commit=False`) and the object is only read.
Report as: `<object>` is read after `<its session ends / commit>` at `<site>`: accessing `<attr>` raises `<DetachedInstanceError / MissingGreenlet>` or runs a hidden query outside the transaction; fix: load what is needed eagerly and return a plain DTO, set `expire_on_commit=False`, or keep the work inside the session scope.
