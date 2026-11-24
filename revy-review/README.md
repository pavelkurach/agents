# revy-review

A code-review skill for agent harnesses, with a small Rust CLI that validates every step. It reviews a change for
what it breaks: a problem gate, two reviewers that trace impact beyond the diff (a second independent draw on
request), an investigator on the strongest model for the doubts they could not settle, one verifier, an editable
report (with unconfirmed suspicions kept separate), and a fix loop for self-review.

- `skill/` — the skill: `SKILL.md` (the orchestrator), the subagent briefs, `rules/`, `examples/`.
- `cli/` — the `revy-review` binary the skill runs.
- `docs/spec.md` — the spec: file formats and what `revy-review check` enforces.

## Pipeline
1. Problem gate (`problem.md`) → `problem.json`; a change that misses its problem stops here.
2. Reviewers (`reviewer.md`, one lens pair each; each changed unit compared with what base guaranteed and probed by running it) and enabled checks
   (`check.md`) → `reviews/<n>.json`.
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
- `reviewers=4` — a second independent draw of both reviewers, for risky changes.

The CLI, run from a review directory (or given it as `[dir]`; each command reads `state.json` there):

```
revy-review rules     # validate the rules folder, print the index for the change's languages
revy-review check     # validate every file in the review directory and the accounting across them
revy-review sync      # render report.md from a clean review, or read back the owner's edits
revy-review comments  # print the post-ready comments from report.md
```

## Custom rules
Rules are plain files, `skill/rules/<topic>/<id>.md`; add a file to a topic folder, or a new folder for a new topic.
No code change. Format in `skill/rules/README.md`, an example in `skill/examples/custom-rules.md`. Optional: checks in
`~/.revy/review/checks/` or `<repo>/.revy/review/checks/`, and your tone in `skill/voice.md`.

## License
MIT — see `LICENSE`.
