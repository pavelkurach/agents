# self-approval-allowed
langs: any

When: Approval or two-person step added, or a new irreversible high-impact action (mass delete, payout, credential issuance, production config push).
Ask: Is approver ≠ requester enforced server-side, or can the initiator approve their own request? Does another path to the action (API, script, bulk endpoint) skip the approval? For a new action, are comparable actions in the codebase gated behind approval while this one is not?
Report as: `<action>` can be executed by one identity `<alone / by self-approval / via <path>>`: one stolen credential or one mistaken operator causes `<irreversible effect>`; fix: require a distinct approver, checked server-side on every path to `<action>`.
