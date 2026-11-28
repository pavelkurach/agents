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

## revy-plan

[**revy-plan**](revy-plan) — plans a non-trivial change evidence-first, before implementation: full impact
traced, every requirement clarified, reviewed by lenses picked for what the change touches. Writes a concise
`plan.md`, a separate `log.md` of how it was built, and hands rendering to a configurable skill.

```
ln -s $PWD/revy-plan/skill ~/.claude/skills/revy-plan
```

## revy-explain

[**revy-explain**](revy-explain) — explains how something works, a task, a pull request, a review report's findings, or a piece of
code — picture first, from first principles, for a reader who arrives cold and should leave as the system's owner.
Every flow is drawn end to end, every claim is backed by evidence, and follow-up questions land in an FAQ.
Rendering goes to a configurable skill, revy-render by default.

```
ln -s $PWD/revy-explain/skill ~/.claude/skills/revy-explain
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
