# revy-plan

An evidence-first planning skill: it traces the full impact of a change, clarifies every requirement, and has
the plan reviewed by lenses chosen for what the change touches — before any code is written.

- `skill/SKILL.md` — the workflow: capabilities → frame → explore → decide → draft → review → converge → present.
- `skill/references/` — evidence, design, reviewer lenses, writing and presentation.

Each plan lives in `~/.revy-plan/plans/<slug>/`: `plan.md` (the plan), `log.md` (evidence, decisions, review
history), `presentation.md` (how to render it).

## Rendering

Rendering is delegated to another skill named in `~/.revy-plan/config.toml`:

```toml
renderer = "visual-plan"
```

The first run asks for it when the file is missing.

## Install

```
ln -s "$PWD/skill" ~/.claude/skills/revy-plan
```

## License

MIT — see `LICENSE`.
