# whole-model-serialized
langs: any

When: Whole model, row or entity serialized to clients or other services: `fields = "__all__"`, `model_dump()`, `to_dict()`, serde on a DB struct, `SELECT *`.
Ask: Which fields go out today, and which does any consumer read? A column added later — secret, internal flag, another tenant's id — ships without anyone touching this code. Is the surface published, so every exposed field becomes a contract to keep?
Not when: a response schema or DTO lists its fields explicitly.
Report as: `<surface>` serializes all of `<model>`: `<field>` goes out unneeded, and every future column leaks by default; fix: an explicit response schema or field list.
