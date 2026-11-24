# naive-datetime
langs: any

When: Timestamp created, parsed, stored or compared without a zone, or days computed as 24 hours.
Ask: Zone-less forms: `datetime.now()`, `utcnow()`, `strptime` without `%z`, `NaiveDateTime`, SQL `timestamp` without time zone. Which zone does each value carry, and where does it meet UTC or local values? Read the column type and the parser. `utcnow().timestamp()` treats the value as local time; JS reads an offset-less date-time as local but a date-only string as UTC. Does day arithmetic cross a DST change?
Report as: `<value>` is naive at `<boundary>` and mixed with `<UTC / local>` values: results shift by the offset, or by an hour at DST; fix: aware UTC at the boundary (`datetime.now(timezone.utc)`, `Utc::now()`, `timestamptz`), and calendar arithmetic in the user's zone.
