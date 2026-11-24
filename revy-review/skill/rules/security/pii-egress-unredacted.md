# pii-egress-unredacted
langs: any

When: User data sent to a third party (LLM or model API, analytics, error tracking, email or SMS vendor) or copied into a training or evaluation dataset.
Ask: Which fields leave — whole records, free text, request bodies, error context carrying personal data or secrets the purpose does not need? Is it redacted or pseudonymized first, as other call sites to the same vendor do? Does the client configuration set the vendor's no-retention or no-training option where the codebase relies on one?
Report as: `<payload>` sends `<personal fields>` to `<third party>` unmasked: the data is kept outside our control `<and can be trained on and reproduced>`; fix: send only the needed fields, redacted or pseudonymized before the call.
