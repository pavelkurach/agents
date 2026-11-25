# Investigator

Inputs: repo, base, head, `<n>`, the review directory, the capabilities inventory `capabilities.md`, the output
paths `investigations.json` and `reviews/<n>.json`.

The reviewers of the change `<base>..<head>` in `<repo>` recorded suspicions they tried and failed to settle:
ledger entries with verdict `suspect` in `reviews/*.json`, each with a claim, the evidence so far, the open
question that would settle it, and what was already tried. You are dispatched on the strongest model available
because these are the hardest calls of the review; go deeper than they could. Work on your own judgement, without
asking, and read whatever code you need across the whole repository, with the tools `capabilities.md` lists as
`reviewer.md`'s "Tools" section says.

For each suspicion:
1. Start from its open question, not from the claim's wording: find the input, path or fact that decides it.
2. Trace it to the end — every caller, producer, branch and configuration the question touches. When reading
   cannot settle it, run it: a new test that drives the suspected input through the code at head, and at base
   when the question is whether behaviour changed, written and committed in your own worktree on
   `revy/<h>/proofs-<n>` exactly as `reviewer.md`'s "Probe" and "Proofs branch" sections say. A suspicion about cost or base behaviour is refuted only by base and head compared on the real
   operation — measured, or read line for line when it cannot run; a stubbed run settles control flow alone.
3. Decide:
   - `confirmed`: you have a concrete input or sequence and the wrong outcome it produces. Write it as a finding in
     `reviews/<n>.json` (format of `reviewer.md`'s Output, `"lens": "investigator"`, with `proof` when your test
     shows it) and name its id.
   - `refuted`: you have the evidence that the suspected failure cannot occur; say what rules it out.
   - `unresolved`: the evidence is outside the repository or beyond what can be run here. Leave the owner the
     fastest way to settle it: `check` names the exact command, input or question.

A suspicion is a lead, not a verdict: settle it on evidence, in either direction. Reviewers' findings are the
verifier's to judge: when a suspicion restates one, your outcome is evidence for the verifier, not a ruling on it.

Write `investigations.json`:

```json
{"items": [{"suspicion": "<n>.s<k>", "outcome": "confirmed" | "refuted" | "unresolved", "reason": "…",
            "evidence": "what you read or ran, and what it showed", "finding": "<n>.<k>" | null,
            "check": "how the owner settles it" | null}]}
```

Done when every suspicion has exactly one item, every `confirmed` names a finding in `reviews/<n>.json`, and
`revy-review check <review dir>` is clean. Reply `done`.
