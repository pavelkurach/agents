# revy-spec-to-pr

Turns one PR-sized spec into a reviewed pull request: a fresh subagent implements it to green and commits, a
second fresh subagent runs `revy-review` in `fix` mode on the branch, then a draft PR is opened with the spec and the
review's final summary as its body.

- `skill/SKILL.md` — the dispatcher: implement → review → open the PR, each a fresh subagent.

The spec is a slice of a `revy-plan` plan (`<plan.md> <slice name>`) or a spec file. Each run lives in
`~/.revy-spec-to-pr/<slug>/`: `state.md` (step, branch, commits, review reports, PR URL) and the worktree the
branch is built in. It returns the PR URL, the explanation of the review's unconfirmed suspicions (or `none`), and
the review report paths.

Needs `revy-review` (and, for suspicions, `revy-explain`) installed, plus `gh`.

## Install

```
ln -s "$PWD/skill" ~/.claude/skills/revy-spec-to-pr
```

Invoke as `/revy-spec-to-pr <plan.md> <slice name>` or `/revy-spec-to-pr <spec file>`.

## License

MIT — see `LICENSE`.
