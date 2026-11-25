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
0. Capabilities → `capabilities.md`: the coordinator inventories code intelligence, runtime execution and search,
   and waits for the owner only when it has an install or enable command to suggest.
1. Problem gate (`problem.md`) → `problem.json`; a change that misses its problem stops here.
2. Reviewers (`reviewer.md`, one lens pair each; each changed unit compared with what base guaranteed and probed by
   running it) and specialists (`reviewer.md` with a focus) → `reviews/<n>.json`.
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

A finding the reviewers could reproduce comes with a new test that fails at head; report.md and the comments show
the test's code, its command and its failure. Those tests live on a local branch `revy/<first 7 of head>/proofs`,
named under report.md's `## Proofs` and never pushed or merged into the reviewed branch: `git checkout` it (or
`git worktree add`) to run them, `git branch -D` it to drop it. The reviewers' own `…/proofs-<n>` branches are
deleted once the verifier has merged the kept proofs; with no proven finding, no proofs branch remains. With `fix`, each proven finding's test moves onto your branch first, fails, then passes with the fix in
the same commit; in team mode the PR branch is never touched.

The CLI, run from a review directory (or given it as `[dir]`; each command reads `state.json` there):

```
revy-review rules     # validate the rules and specialists, print the rules index
revy-review check     # validate every file in the review directory and the accounting across them
revy-review sync      # render report.md from a clean review, or read back the owner's edits
revy-review comments  # print the post-ready comments from report.md
```

## Extending
Two things extend a review: **rules** (one question a reviewer must answer where the diff matches a condition) and
**specialists** (an extra reviewer subagent with its own focus). Built-ins live in the skill: `skill/rules/` and
`skill/reviewers/` (e.g. `migrations.md`). Yours go in `~/.revy-review/`, which every review loads after the
skill's; without it, only the built-ins run.

```
~/.revy-review/
├── README.md                 # optional; anything outside rules/ and reviewers/ is ignored
├── rules/
│   └── <topic>/<id>.md       # one rule per file; the folder is its topic, the file name its id
└── reviewers/
    └── <name>.md             # one specialist per file; the file name is its name
```

**A rule**, `~/.revy-review/rules/payments/amount-as-float.md` (full format: `skill/rules/README.md`). Only `When:` is
required; `langs:` is an optional free-text hint (`python`, `java, kotlin`) that reviewers read to skip a rule that
does not apply to the change. `When: always` applies it to every changed file.

```markdown
# amount-as-float
langs: python, typescript

When: The diff stores, computes or serializes a money amount.
Ask: Is the amount an integer of minor units or a decimal type end to end, including JSON and the database column?
Not when: display-only formatting of an amount that is never stored or summed.
Report as: `<amount>` is a float, so `<operation>` rounds cents; fix: integer minor units or Decimal end to end.
```

The four core reviewers receive every rule, of every topic; findings cite a rule by its id.

**A specialist** runs after the four core reviewers, as reviewer 5, 6, … (built-ins first, then yours, file-name
order, counting only the ones dispatched). Its file starts with optional header lines in any order — `rules:
<topic>, …` and `dispatch: always | when relevant` — then the prompt: what it reviews. It reviews only that, and
writes an empty review when the change has nothing for it. `always` (the default) runs it on every review; `when
relevant` lets the coordinator skip it when the change holds nothing for its prompt (when unsure, it runs), and
report.md lists each skip with its reason under `## Specialists skipped`.

With rules, `~/.revy-review/reviewers/payments.md` receives the rules of those topics, from both rules folders:

```markdown
rules: payments, security

Payment flows: every change that charges, refunds or moves money. Check idempotency keys on every retry path,
that amounts never pass through floats, and that each state change of a payment is written to the ledger table
in the same transaction.
```

Without a `rules:` line, `~/.revy-review/reviewers/feature-flags.md` gets no rules, only its prompt, and runs only
on changes that touch flags:

```markdown
dispatch: when relevant

Feature flags: every flag the change adds, reads or removes. Check that the default is the old behaviour, that
both branches are tested, and that a removed flag has no remaining reads in code, config or dashboards.
```

**Sharing a company pack**: keep the `~/.revy-review/` tree in a git repository; each engineer runs
`git clone <url> ~/.revy-review` once and `git -C ~/.revy-review pull` to update.

**Validate** with `revy-review rules` (from a review directory): it prints the rules index, then
`Specialist: <name> (dispatch: <mode>) — <file>` per specialist with its rules below it. It exits 1 and prints, per broken file:
- `<file>: rule id <id> is also <other file>` — rule ids are unique across both rules folders and all topics.
- `` <file>: rule <id> has no `When: …` line ``.
- `<file>: specialist <name> is also <other file>` — names are unique across `skill/reviewers/` and yours.
- `` <file>: specialist <name> names rule folder `<topic>`, which no rules directory has `` — a typo, or a topic
  with no rules.
- `<file>: specialist <name> has no prompt; write what it reviews`.
- `` <file>: specialist <name>: `dispatch: <value>` is not `always` or `when relevant` ``.

Fix every error before reviewing; a rule or specialist file that does not parse is left out of the index.

Your tone for posted comments goes in `skill/voice.md`.

## License
MIT — see `LICENSE`.
