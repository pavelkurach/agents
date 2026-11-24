# personal-data-outside-erasure
langs: any

When: Column, table, log field, cache, file or search index added that holds personal or sensitive data, or a new copy of such data.
Ask: Does the existing deletion, export and retention path (account deletion, erasure request, TTL job) cover the new location — search for where existing user data is deleted? Is the raw value needed, or would an id, hash, truncation or aggregate do? What bounds how long it is kept?
Report as: `<location>` stores `<data>` that `<erasure path>` never removes `<and keeps without limit>`: deletion requests leave it behind and every breach exposes it; fix: store the minimal form, add the location to `<erasure path>`, and set a retention bound.
