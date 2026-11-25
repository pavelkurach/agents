# revy-rules-ingest

A skill that turns a source into [revy-review](../README.md) rules: a book or chapter (PDF, EPUB, Markdown), an
article or URL, a postmortem, or a PR whose bug escaped review. It keeps only rules that are effective, concise and
generic, has a fresh subagent critique each draft, shows you every proposed rule and every cut, and writes files only
after you approve them: to `~/.revy-review/rules/` by default, or to the shipped `skill/rules/` when you say a rule is
generic enough. When the source covers a whole domain it can also propose a specialist reviewer.

Install (from the repository root):

```sh
ln -s $PWD/revy-review/ingest ~/.claude/skills/revy-rules-ingest
```

Example:

```
/revy-rules-ingest ~/books/designing-data-intensive-applications.epub chapter 7
```

`scripts/rules.sh` lists every rule (`index`) and validates rule files (`lint`, `check`); `check` needs the
`revy-review` CLI on `PATH`.
