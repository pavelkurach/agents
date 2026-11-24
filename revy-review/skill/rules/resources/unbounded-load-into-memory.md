# unbounded-load-into-memory
langs: any

When: Whole result set, file, upload, body or stream loaded into memory, fetched rows filtered or counted in app code, or a list query added without a limit.
Ask: What bounds the size — `LIMIT` or pagination with a capped page size, a body or upload limit — and can a client raise it (`?limit=` with no ceiling)? Does the size limit apply after decompression (gzip bodies, zip archives, images), or only to the compressed bytes? Could the filter, `COUNT`, `ORDER BY … LIMIT` or `DISTINCT` run in the query instead of over `.all()`? Is the data consumed once, so a cursor, iterator or chunked stream could replace `.all()`/`fetchall()`, `read()`, `list(gen)`, `collect()`?
Not when: the source has a small fixed bound · the store cannot express the condition.
Report as: `<site>` holds all of `<source>` in memory `<with no bound / to keep only <subset>>`: as `<data>` grows the request `<times out / OOMs the worker>`; fix: push `<filter / count / limit>` into the query, paginate with a capped page size, stream in chunks, or enforce a size limit at the boundary, after decompression.
