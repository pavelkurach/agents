# read-own-write-from-async-copy
langs: any

When: Write followed by a read of the same data through a replica, secondary index, search index or cache that is updated asynchronously.
Ask: Is the read source updated after the write commits — a read replica, a DynamoDB GSI (eventually consistent reads only), Elasticsearch or OpenSearch before refresh, a cache filled by a change stream? Does the code or user expect to see the write at once: a redirect after create, a uniqueness check, a follow-up step?
Not when: the read tolerates staleness · the read goes to the primary or uses a strongly consistent option.
Report as: `<read>` after `<write>` goes through `<async copy>` and can miss the write: `<the user sees it vanish / the check passes wrongly>`; fix: read from the primary or with a consistent read, or carry the written value forward.
