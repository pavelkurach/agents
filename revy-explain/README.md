# revy-explain

An evidence-first explaining skill: it answers *how does X work*, and lays out the full context of a task, of a
pull request to review, of every finding in a review report, or of a piece of code — picture first, for a reader with no prior
context, every claim backed by evidence. Follow-up questions are recorded in the explanation's FAQ.

- `skill/SKILL.md` — the workflow: frame → explore → draft → verify → render, plus follow-ups.
- `skill/references/` — writing and look, evidence, and one shape per mode (`how-it-works`, `task`,
  `pull-request`, `review-report`, `code`).

Each explanation lives in `~/.revy-explain/explanations/<slug>/`: `explanation.md` (the content, Mermaid
diagrams included), `log.md` (status, request, evidence ledger), `render/` (the rendered page).

`revy-plan` reads a matching explanation as the user's baseline: its claims are leads it re-proves, and each
planning question carries only the context the explanation left out.

## Rendering

Rendering is delegated to another skill named in `~/.revy-explain/config.toml`:

```toml
renderer = "revy-render"
```

`skill/references/writing.md` follows revy-render's markdown conventions; another renderer may miss them. The first run asks for it when the file is missing.

## Install

```
ln -s "$PWD/skill" ~/.claude/skills/revy-explain
```

Invoke as `/revy-explain <question | task | PR | report | code>`.

## License

MIT — see `LICENSE`.
