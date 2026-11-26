# sensitive-data-in-log-or-response
langs: any

When: Value that may carry secrets or personal data (body, headers, token, record) logged, traced, raised or returned, or such a field added to a type.
Ask: What does the formatted value contain? Read its `__repr__`, `Debug`, `toString` or serializer, including derived ones (`@dataclass`, `#[derive(Debug)]`, model dumps) that print every field. If a type gained a secret field, search where that type is logged or serialized. Do client-facing errors carry internal detail (SQL, stack, hostnames, raw records)? In CI logs, runner masking matches only the exact value of a registered secret: a base64- or URL-encoded, JSON-wrapped, split or derived value (a JWT signed with the key) prints in clear unless registered with `::add-mask::`.
Not when: the field type already redacts (`SecretStr`, `secrecy::SecretString`, `field(repr=False)`) · a CI secret printed unchanged in a job that references it, which the runner masks.
Report as: `<site>` writes `<secret / PII>` into `<log / trace / response>`: it spreads to every system and person with access to those and stays for their retention; fix: a redacting type for the field, identifiers instead of payloads in logs, generic client errors with a correlation id.
