# Check

Inputs: repo, base, head, `<n>`, the check file's path, `<problem criteria>`, the rules index, the output path
`reviews/<n>.json`.

You run one check on the change `<base>..<head>` in `<repo>`, alongside the reviewers. The problem it solves:
`<problem criteria>`. Work on your own judgement, without asking, and read whatever code you need across the whole
repository.

Read the check file: its brief is your task, all of it, and its done criterion is when you may stop. Read
`reviewer.md` in this skill's directory and work as its "Impact beyond the diff" and "How to look" sections say
wherever the brief touches them, and apply every rule whose `When` is `always` to every changed file.

## Output
Write `reviews/<n>.json` in the format of `reviewer.md`'s Output, with `"lens"` = the check's name and one ledger
entry per site the check examined.

Done when the check's own done criterion is met and every changed file has at least one ledger entry.
`revy-review check` validates the file as it does a reviewer's; you may be sent its errors to fix. Reply `done` when
the file is written.
