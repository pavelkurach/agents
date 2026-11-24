# revy-review

A code-review skill for agent harnesses, with a small Rust CLI that validates every step. It reviews a change for
what it breaks: a problem gate, four reviewers that trace impact beyond the diff (two lens pairs, each drawn twice),
specialists that each review one focus, an investigator on the strongest model for the doubts they could not
settle, one verifier, an editable report (with unconfirmed suspicions kept separate), and a fix loop for
self-review.

- `skill/` — the skill: `SKILL.md` (the orchestrator), the subagent briefs, `rules/`, `reviewers/` (specialists),
  `examples/`.
- `cli/` — the `revy-review` binary the skill runs.
- `docs/spec.md` — the spec: file formats and what `revy-review check` enforces.

## Pipeline
1. Problem gate (`problem.md`) → `problem.json`; a change that misses its problem stops here.
2. Reviewers (`reviewer.md`, one lens pair each; each changed unit compared with what base guaranteed and probed by
   running it), specialists (`reviewer.md` with a focus) and enabled checks (`check.md`) → `reviews/<n>.json`.
3. Investigator (`investigator.md`) for the suspicions they left → `investigations.json`.
4. Verifier (`verifier.md`) → `verified.json`; `revy-review sync` renders `report.md`, which the owner edits.
5. `revy-review comments` prints the post-ready comments; in self-review, `fix` fixes the findings and re-reviews.

`revy-review check` runs after every step and must be clean before the next one.

## Install
From this folder:
1. CLI: `cargo install --path cli` (puts `revy-review` on PATH).
2. Skill: symlink the skill folder into your harness's skills directory, e.g.
   `ln -s "$PWD/skill" ~/.claude/skills/revy-review`.

## Use
- `/revy-review` — review the current branch; `/revy-review <PR/MR | commit | A..B>`.
- `/revy-review fix` — self-review and fix everything (one commit per round, never pushed).

The CLI, run from a review directory (or given it as `[dir]`; each command reads `state.json` there):

```
revy-review rules     # validate the rules and specialists, print the index for the change's languages
revy-review check     # validate every file in the review directory and the accounting across them
revy-review sync      # render report.md from a clean review, or read back the owner's edits
revy-review comments  # print the post-ready comments from report.md
```

## Extending
Built-in specialists live in `skill/reviewers/`; your own rules and specialists go in `~/.revy-review/`, which
every review reads on top of the skill's (absent: built-ins only):

```
~/.revy-review/
├── rules/
│   └── <topic>/<id>.md       # same layout and format as skill/rules/
└── reviewers/
    └── <name>.md             # one specialist each
```

A company can ship this folder as a git repository: `git clone <url> ~/.revy-review`.

A rule, `~/.revy-review/rules/payments/amount-as-float.md` (format in `skill/rules/README.md`; rule ids are unique
across both rules folders, a duplicate is an error naming both files):

```markdown
# amount-as-float
langs: python, typescript

When: The diff stores, computes or serializes a money amount.
Ask: Is the amount an integer of minor units or a decimal type end to end, including JSON and the database column?
Report as: `<amount>` is a float, so `<operation>` rounds cents; fix: integer minor units or Decimal end to end.
```

A specialist, `~/.revy-review/reviewers/payments.md`: an optional `rules:` line naming the rule topics it receives
(without it, none), then its prompt. Every specialist runs on every review, after the four reviewers, and reviews
only what its prompt covers; the file name is its name, unique across `skill/reviewers/` and yours.

```markdown
rules: payments, security

Payment flows: every change that charges, refunds or moves money. Check idempotency keys on every retry path,
that amounts never pass through floats, and that each state change of a payment is written to the ledger table
in the same transaction.
```

`revy-review rules` validates both and lists each specialist with its number. Optional: checks in
`~/.revy/review/checks/` or `<repo>/.revy/review/checks/`, and your tone in `skill/voice.md`.

## License
MIT — see `LICENSE`.
