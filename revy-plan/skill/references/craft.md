# Craft

Applies to every artifact: how a claim is cited, and how the prose reads.

## Clear and lean

Write in plain, direct language. Central reasoning — why a design is shaped the way it is, the one risk
that carries the plan — earns prose. Supporting detail — file lists, exhaustive constraints, field
enumerations — goes in compact structured form: lists, tables, short bullets, or a diagram, never a wall
of prose. A reader skims the structure and reads the few sentences that carry the decision. If a
paragraph runs past a few sentences on non-central detail, it is a list. A merge request's
`standalone_behavior`, an acceptance criterion, a figure `note`: one clear sentence, not a design essay.

**Prefer a diagram once a flow has more than a step or two.** A strong default, not a rule: keep prose
only for genuinely single-step behavior — "reads the file, returns its bytes" is fine as a sentence.
Once a flow has several steps, an on-success/on-error branch, or a call chain of roughly three or more
hops, draw it as an authored flow/sequence/state Figure — the current-behavior flow and the
target-behavior flow. When you do, the renderer leads the behavior section with those figures and keeps
the current/target prose fully visible, full-width, directly below them — never folded away. Each
behavior field then stays a tight supporting summary: the figure already carries the multi-step flow, so
a sentence is enough — brief because the diagram shows the detail, not because the renderer hides the
prose. Orientation's touched flows follow the same threshold:
draw them (a Figure, or the as-is call graph from `CallEdges` evidence) once they are more than a hop
or two.

Don't narrate a multi-step or branchy flow in prose — draw it:

| Multi-step prose to draw instead | Draw |
|---|---|
| `revy-plan serve --port P` → `run_serve` → `serve::run` → `bind(P)` | a flow Figure with those steps as nodes |
| "on success wraps the listener; on any error returns `ServeError::Bind`, propagated by its one caller with `?`" | a flow Figure branching success/error, or a state Figure |

When a change alters a function, struct, or enum signature, show the before and after signature as a
fenced code block (`struct RenderArgs { … quiet: bool }`, `fn run_render(args: &RenderArgs) -> ExitCode`),
not a sentence. The renderer never invents a diagram from prose — no figure, no picture.

The plan states the design, not its own history. Do not author a "what review changed" / "corrections"
section: a reader wants the plan that survived review, not a diff against an earlier draft. Fold what
review taught into the design itself.

**Code renders as code.** The renderer reads every prose field as Markdown and auto-monospaces the
high-confidence shapes for you — a filename with a code extension (`main.rs`), a `file.rs:NNN` anchor, a
`snake_case` name, a `macro!`, an `a::b::c` path, an `a/b/c.rs` path, a `revy-plan <sub>` command. What it
cannot judge, you must: a bare `CamelCase` or single-word symbol (`CorrectionResult`, `Store`) is
ambiguous, so wrap it in backticks yourself. A symbol citation, a decision naming a type, an impact
rationale naming a function: backtick it.

## Citations

Every claim about existing behavior carries one. No citation → it is a loose end, not a fact.

**Cite symbols, not lines.** `src/lib.rs::Store::add`. A symbol breaks only on rename or delete, which
is itself worth surfacing.

| Thing | Form |
|---|---|
| Ambiguous name in one file | qualify — `src/lib.rs::impl Add for Store::add` |
| Resolved through inheritance | where it is **defined**, plus the dispatch site |
| A branch or statement inside a function | enclosing symbol + the literal |
| A match arm | enclosing symbol + the variant |
| A config key, a markdown section, a flag | `path` · the key or heading |
| Executed behavior | the command and its output, inline |
| Generated code | the **generator input**, never the output |

## Evidence markers

One vocabulary for how you know — the only confidence marker, used in citations, challenges, decision
cells, and evidence. **The marker is the class of evidence; the tool goes in parentheses after it**,
named by what it proved rather than what it is. A resolved edge is `via:lsp` whatever produced it; an
execution is `via:runtime`; a text match is `via:grep`; anything else is `via:tool(name)`.

| Marker | Means |
|---|---|
| `via:lsp` | Proven edge — call hierarchy, references, definition |
| `via:runtime` | Executed **this** repo, output pasted |
| `via:prototype` | Built the proposed design in scratch and ran it; say what and where |
| `via:doc` | A cited spec, RFC or vendor document |
| `via:tool` | Any other code-intelligence tool, named — `via:tool(ast-grep)`, `via:tool(ctags)`. Better than a text match, short of a resolved edge |
| `via:read` · `via:grep` | Found textually, edge unproven — a flag, not a defect. Usually dynamic dispatch, a decorator, a registry, a callback: where designs break |
| `via:inference` | Believed, not demonstrated. Carries nothing |

**Prefer `via:tool` to `via:grep` for anything you are going to count** — a text search inflates a count
with comments and strings. A count a decision rests on carries two independent methods, and the row
carries both markers. A claim about a boundary is run on both sides and both are pasted, labelled by
repo and interpreter version. **Never retype a transcript, and never carry one across sections** — paste
it as it came. **Re-verify a correction like the claim it replaced**; it was fixed under pressure.

**A runtime-evidence transcript lives in the plan's transcripts home, not the target repo.** A
`Runtime` evidence's `artifact` path is resolved against the external `<plan-home>/transcripts/`
directory (a sibling of `plan.json`), not the checkout — so write the captured output there and reference
it by a bare relative name (`probe.txt`), never a `target/…` or source path. The CLI verifies the pasted
digest against that file; a missing one's error names the absolute path it looked for, which is where the
file belongs.

**Use your runner's no-mutation flag** — `uv run --no-sync` or the ecosystem's equivalent.

## An absence is a claim

It carries the marker of the search that failed — `via:grep` for *no config key matches*, a resolved
`via:lsp` for an empty symbol query — and it is written as a sentence, not as silence: *no `impl Drop`
anywhere in the crate*. One left unsaid is indistinguishable from one never looked for.

| Shape | Example |
|---|---|
| No match anywhere | no feature flag on this path |
| The symbol exists and nothing references it | `deallocate_page` is reachable only from tests |
| **Nothing writes this field after it is set** | no writer rewrites the amount after creation |

The third decides whether a value can be trusted downstream, and a text search answers it badly.

## Prose

**Write for someone who has never opened this repository** — the implementer picking up one slice, and
the requester specifying work in code they have never read.

Banned:

- Agreement openers — "Great point", "You're right", "Good catch".
- Throat-clearing — "It's worth noting", "Note that", "As mentioned above".
- Restating the section header as its first sentence; closing summaries that repeat the bullets above.
- **Rationale paragraphs.** State the rule; do not then explain why it is good.
- **Revision language** — comparisons with superseded designs or abandoned options.
- `robust` · `clean` · `scalable` · `efficient` · `elegant` — legal only with a number or a citation.
- `TBD`, "handle edge cases", "add appropriate error handling" — name the cases.
- A type or function named in a call stack and defined nowhere.

**Omit empty sections.** Where silence is ambiguous, one explicit line instead: `no concurrency
surface`, `no new seams`, `no migration`.

## Straight to the point

**The finding first, then the evidence.** A section opens with the fact the change turns on, not the
first thing you read.

**Anything enumerable is a table.** Three or more of anything — cases, consumers, statuses, steps — is
never a paragraph. Every table carries a one-line lead-in saying what is being enumerated and the shape
of the answer, never the heading restated. **A cell is a clause, not a paragraph.**

**A paragraph longer than four lines is a diagram you have not drawn yet**, unless it is a decision, a
tradeoff or a falsifier. Ordering, branching, layering, containment and proportion are all drawable —
see [presentation.md](presentation.md) for which shape fits which content.

**Say nothing twice.** **Cut words, never findings** — none of this licenses dropping a case, a
consumer or a branch. **Diff size never reduces depth:** a one-line fix in a flow crossing two services
receives the same complete evidence, behavior, contract, and failure-path treatment as any other
change. Name the specific cut you made, or the specific thing you tried to cut and why it survived.

## Skill contradictions

A rule in this skill you could not satisfy while planning — two rules that conflict, or one ambiguous
enough that you had to guess — is a skill contradiction. Cite it by file and line, say what the rule
made you do instead, and never resolve it yourself by picking a side.

**Surface it to the user** — [review-loop.md](review-loop.md) § Gate report, `skill contradictions:`.
This is the loop that fixes the skill: a rule the planner could not satisfy is a defect in these
references, not in the plan.
