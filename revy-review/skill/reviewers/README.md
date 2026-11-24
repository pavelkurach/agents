# Specialists

A specialist is an extra reviewer with one focus. Every file here, and every file in `~/.revy-review/reviewers/`,
runs on every review after the four core reviewers, as reviewer 5, 6, … (this folder first, then yours, each in
file-name order). This README is not one.

## Format
`<name>.md` — the file name is the specialist's name, unique across both folders.

```markdown
rules: migrations, data

Schema and data migrations the change adds or alters, and the code they meet during the deploy: …
```

- `rules: <topic>, …` (optional first line) — the specialist receives only the rules in those topic folders of
  `skill/rules/` and `~/.revy-review/rules/`. Without the line it receives no rules, only its prompt.
- The rest is the prompt: what it reviews. It is read on top of `../reviewer.md`, so it states the focus, not the
  method or the output format. The specialist reviews only what its focus covers, and writes an empty review when
  the change has nothing for it.

## Built-in
- `migrations.md` — schema and data migrations: non-blocking, idempotent, safe to rerun after a partial failure.

## Adding one
Put company-specific specialists in `~/.revy-review/reviewers/`; add one here only when its focus is generic.
`revy-review rules` (from a review directory) lists every specialist and its rules, and names any broken file.
