# revy-solve-ticket

The full revy flow on one ticket, with the user keeping full understanding and every decision:

1. **Explain** — `revy-explain` in `task` mode; the user's questions land in its FAQ until they say they
   understand.
2. **Plan** — `revy-plan`, with the explanation as the user's baseline; its questions are relayed to the user,
   later questions land in the plan's FAQ, until the user approves the plan.
3. **Implement** — wave by wave from the plan's slice graph: one `revy-spec-to-pr` per slice, in parallel; the next
   wave starts only after the user says the current wave's PRs are merged.

- `skill/SKILL.md` — the dispatcher: dispatch and relay, the checkpoint, the three stages and their gates.

Subagents cannot reach the user, so the dispatcher relays every question and resumes the same agent with the
answer. Progress is checkpointed in `~/.revy-solve-ticket/<slug>/state.md`, so a new session resumes it.

Needs `revy-explain`, `revy-plan`, `revy-spec-to-pr` and `revy-review` installed, plus `gh`.

## Install

```
ln -s "$PWD/skill" ~/.claude/skills/revy-solve-ticket
```

Invoke as `/revy-solve-ticket <ticket | issue URL | description>`.

## License

MIT — see `LICENSE`.
