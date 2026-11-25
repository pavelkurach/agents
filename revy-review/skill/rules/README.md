# Review rules

One rule per file, `<topic>/<id>.md`: every folder here is a topic, and the file name is the rule id that findings
cite. Every line must change what a reviewer does; anything a strong reviewer already knows, and anything about
where the rule came from, stays out.

```markdown
# <id>
langs: <hint, free text: python | java, kotlin>      (optional)

When: <what is visible in the diff that makes this rule apply — one sentence; it becomes the index line>
Ask: <the question to answer, and what to read or search to answer it; up to 3 sub-questions about the same site>
Not when: <look-alikes that are fine> · <another>      (optional)
Report as: <the root defect and its consequence, with placeholders>; fix: <the deepest fix the evidence supports>
Example:            (optional; mostly language packs)
  bad:  <1–2 lines>
  good: <1–2 lines>
```

- `revy-review rules` reads only `When:` (required) and `langs:` (optional) and lists every rule. `langs` is a
  free-text hint, never interpreted: reviewers skip a rule whose languages the change does not touch. Omit it for
  a rule that applies to any language. The rest of the file is for the reviewer who opens it.
- `When` names a change in the diff, not a principle; keep it to one short sentence. `Ask` is checkable with evidence.
- `When: always` asks reviewers to apply the rule to every changed file.
- Write `Ask` and `Report as`. `Not when` only for a rule with a known look-alike that reviewers would otherwise
  flag; `Example` only where the wording alone is ambiguous (mostly language idioms), since a model anchors on it.
- No severity field: severity follows from the consequence in `Report as`. Where it depends on context (request path
  vs background job), `Ask` asks about it. API names and lists belong in `Ask`, not `When`.
- Topics hold general rules (`concurrency`, `api`, `data`, …) and language-only rules (`go`, `typescript`, `python`,
  `rust`). A rule that fits a general topic goes there even if its examples are language-specific.

Your own rules go in `~/.revy-review/rules/<topic>/<id>.md`, same format, merged with these (example:
`../examples/custom-rules.md`). Ids are unique across all topics of both.
