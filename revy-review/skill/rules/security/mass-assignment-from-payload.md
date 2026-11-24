# mass-assignment-from-payload
langs: any

When: Request, message or webhook payload mapped onto a stored entity or passed raw into a store, or a new entry point writing a resource its siblings also write.
Ask: Is the whole payload spread onto the entity — `Model(**data)`, `obj.update(**body)`, `setattr` in a loop, deserializing straight into the DB struct — so a client can set `role`, `owner_id`, `tenant_id` or `price`? Is the input parsed into a model of exactly the writable fields before the first sink? Does this path skip the parsing, size limits or signature check that sibling entry points for the same resource apply — read them?
Not when: a request model (Pydantic, a `serde` DTO, a zod schema) declares exactly the writable fields and is mapped explicitly.
Report as: `<input>` reaches `<sink>` at `<site>` unparsed: a client can write `<field / shape / size>` and `<escalate / corrupt stored data / break downstream consumers>`; fix: parse into a DTO of the writable fields at the entry point and map it explicitly onto the entity.
