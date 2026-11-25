# Specialists

A specialist is an extra reviewer with one focus. Every file here, and every file in `~/.revy-review/reviewers/`,
is one (this folder first, then yours, each in file-name order); this README is not one. The ones a review
dispatches run after the four core reviewers, as reviewer 5, 6, … in that order.

## Format
`<name>.md` — the file name is the specialist's name, unique across both folders.

```markdown
rules: migrations, data
dispatch: when relevant

Schema and data migrations the change adds or alters, and the code they meet during the deploy: …
```

Optional header lines at the top, in any order:
- `rules: <topic>, …` — the specialist receives only the rules in those topic folders of `skill/rules/` and
  `~/.revy-review/rules/`. Without the line it receives no rules, only its prompt.
- `dispatch: always | when relevant` — `always` (the default without the line) runs it on every review. `when
  relevant` lets the coordinator skip it when it judges, from the prompt and the change, that the change holds
  nothing for it; when unsure it dispatches. report.md lists each skip with its reason. Use it for a focus most
  changes never touch.

The rest is the prompt: what it reviews. It is read on top of `../reviewer.md`, so it states the focus, not the
method or the output format. The specialist reviews only what its focus covers, and writes an empty review when
the change has nothing for it.

## Built-in
- `migrations.md` (`when relevant`) — schema and data migrations: non-blocking, idempotent, safe to rerun after a
  partial failure.

## Adding one
Put company-specific specialists in `~/.revy-review/reviewers/`; add one here only when its focus is generic.
`revy-review rules` (from a review directory) lists every specialist with its dispatch mode and rules, and names
any broken file.
