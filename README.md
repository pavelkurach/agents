# agents

Agent tooling I use day to day.

## revy-review

[**revy-review**](revy-review) — reviews a change for what it breaks: a problem gate, reviewers that
trace impact beyond the diff and probe edge cases against base, an investigator for unsettled doubts,
a verifier, and an editable report. A curated rules base carries the non-obvious knowledge; a small
Rust CLI validates every step.

```
cargo install --path revy-review/cli
ln -s $PWD/revy-review/skill ~/.claude/skills/revy-review
```

## summarize-technical-book

[**skills/summarize-technical-book**](skills/summarize-technical-book) — turns a technical PDF or
EPUB into a decision-focused engineering notebook: concise context, durable learnings, transferable
concepts, selective technologies/APIs, critical commentary, and a deduplicated book-wide synthesis.
Ships its own extraction, schema, assembly, and validation tools.

```
ln -s $PWD/skills/summarize-technical-book ~/.claude/skills/summarize-technical-book
```

MIT.
