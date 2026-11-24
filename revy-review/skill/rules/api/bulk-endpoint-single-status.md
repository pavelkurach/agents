# bulk-endpoint-single-status
langs: any

When: Bulk endpoint or job (create, update or import over a list) added or changed that answers one status for the whole batch.
Ask: Do items commit one by one or in one transaction? After a mid-batch failure, can the caller tell which items applied, and does retrying the whole batch re-apply them or stall on the one bad item? Are duplicates reported as errors or as skipped?
Not when: the batch runs in one transaction and its contract is all-or-nothing.
Report as: `<endpoint>` commits items one by one but answers `<status>` for the batch: callers cannot tell what applied, and a retry `<duplicates committed items / stalls on one bad item>`; fix: per-item results (status, error) with counts, or one transaction.
