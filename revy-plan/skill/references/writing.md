# Writing

Everything on the page must be understood where it stands, and every line must matter. Write **crystal clear**: plain English, the fact that matters first, nothing the
reader does not need to approve or build the change.

## plan.md

Content only — diagrams included, as Mermaid, since their nodes and edges are claims reviewers check. How the
plan was built lives in `log.md`; how it looks lives in `presentation.md`.

Sections, in order — what the reader needs to decide first, implementation detail next, reference last. Leave
out a section with nothing in it, or write one line where silence would be ambiguous (`no migration`,
`no concurrency surface`).

1. **Title + goal** — a noun-phrase title (≤ 60 chars); the goal in one sentence.
2. **Scope** — what changes and what stays as it is, as two short lists.
3. **Behavior** — current → target, plus concrete examples as given / when / then rows: one row per distinct
   behavior, cases that behave alike merged into one row.
4. **Owner's view** — the invariants, limits, intricacies, and rationale (evidence.md § Owner's depth) that bear on
   this change, one table row each; an invariant row says what enforces it and how this change keeps it.
5. **Decisions** — the table from design.md; one-way decisions first.
6. **Risks and accepted unknowns** — each risk with its mitigation; each unknown with the user's acceptance; the
   blind spot. A question that would change the design goes back to the user before the plan is presented.
7. **Changes** — one row per file: its path, its status (added / modified / removed), what it holds (one clause),
   and what changes in it (one clause); then types and signatures before → after as code,
   and the touched flow drawn once: the to-be flow with new and changed edges marked, every failure path
   included.
8. **Delivery** — the dependency graph of slices, then one block per slice. Order is by wave — `wave 1`, `wave 2`,
   … — never by days or other time estimates.
9. **Acceptance and tests** — one row per criterion: the criterion as a sentence; how it is verified — the test
   that proves it, with its status (`new` / `changed` / `existing`) and name, or a command or manual check; what
   that check protects (the bug, contract, or regression); the slice that delivers it. This column is the only
   link between slices and criteria.
10. **Orientation** — placement, and a glossary as `- term: definition` bullets under a `Glossary:` label; the
   renderer explains each term on hover wherever it appears.
11. **Impact** — one row per lane: affected / unaffected / n/a, and why.
12. **Start here** — the one first action of the first slice, doable in minutes: a file to open, a test to write,
    a command to run. Starting is the hardest step; make it obvious.
13. **FAQ** — present once the user has asked a question about the plan: one `###` entry per question, oldest
    first, headed by the question verbatim. Picture first: a Mermaid diagram when the answer has steps,
    branches, or structure; a table when it compares options; a located snippet with chunk notes when it is
    about code; then the fewest words that finish the answer. Cite evidence ids, glossary terms, and code
    references throughout. End with `Changed:` naming the sections a resulting patch touched, when one did.

### Plain English

- Name everything by what it is, in words. A slice is named by what it delivers (*Add the `--json` flag*), a
  criterion is its sentence, a decision is its question — and that name is how the rest of the plan refers to
  it.
- No ids, slugs, codes, or abbreviations the reader must decode — no `S1`, `AC-4`, `empty-input-rejected`.
  Code identifiers, paths, and commands are fine: they are what the implementer types. Backtick them.
- The one id allowed: a claim about existing code ends with its evidence id from `log.md` § `Evidence`
  (*`list` never sorts E3*). The renderer explains it on hover and links it to the full entry.
- Code references point at lines where it matters: `path:L245-L260` or `path::symbol`.
- A term a newcomer would look up goes in the glossary, or gets replaced by plain words.

### Markers

Fixed words carry meaning a renderer can colour; use exactly these:
- a line or bullet starting `Risk:`, `Accepted:`, `Blind spot:`, or `Mitigation:`
- status words, as a whole table cell or the cell's first word: `new` · `changed` · `existing` (tests);
  `added` · `modified` · `removed` (files); `one-way` · `reversible` (decisions); `affected` · `unaffected` ·
  `n/a` (impact); `backward-compatible` · `feature-flag` · `scaffolding` · `breaking` (slices)
- file paths in backticks, relative to the repository root, `path` or `path::symbol`, so a renderer can link
  them to the repository
- in Mermaid, semantic classes instead of colours: `:::new`, `:::changed`, `:::removed`, `:::risk`, `:::failure`,
  `:::oneway`, `:::context`, `:::ok`. Colour every diagram wherever colour carries meaning: the success path
  `ok`, error paths `failure`, and what the change adds, alters, or removes `new` / `changed` / `removed`

### Lines that stand alone

- Every line stands alone: no "as above", no "keep in mind", no pronoun pointing at a paragraph out of view.
- Each slice says what works once it merges, in concrete terms (*`check --json` prints one error per file*).
- A related issue outside scope goes in a `Separately:` list at the end, never inline.
- Literal words: no idioms; a hedge only where the uncertainty is real, and then say what is uncertain.
- Problems as cause and fix, matter-of-fact.
- Test each section: a reader who reads only its first and last line knows what it says and what follows.

### Lean

- Each line passes one test: does the reader need it to approve or build the change? A line about how the plan
  was found, what was considered on the way, or what review changed belongs in `log.md`.
- One fact per line. A paragraph is at most two sentences, and only for a reason or a trade-off.
- Lead each section with the fact the change turns on.
- Two or more items are a list — one item per line, never joined by commas inside a sentence or a bullet
  (files, symbols, flags, cases, steps alike); three or more items sharing fields are a table; a cell is a
  clause.
- A group past five items puts the most important first; `presentation.md` folds the rest.
- A flow of three or more hops, any branch, a lifecycle, or a layering is a Mermaid diagram, failure edges
  included. A one- or two-step behavior is a sentence.
- Concrete over general: *empty list returns `[]`*, *adds 2 files, changes 4*.
- Say each thing once. Cut words, never cases, consumers, or failure paths.

## presentation.md

Instructions for a renderer that knows nothing about the plan, written for any renderer. It decides how the
page looks; `plan.md` decides what it says, and the renderer keeps every item.

Everything it names already exists in `plan.md`; content the look needs goes into `plan.md` first. Write:
- **Reading order** — every `plan.md` section, in the order the reader should meet them, and which open first
  versus fold (orientation, impact, and the tail of any group past five items fold; FAQ entries collapse).
- **Per section** — its visual form: table, list, code block, callout, before/after, tree, cards.
- **Diagrams** — per `plan.md` diagram, by section: its preferred drawing (flow, sequence, state, layers, …) and
  what to highlight (new, changed, removed, one-way). Its nodes and edges stay in `plan.md`.
- **Interactivity** — what the reader should be able to explore: steps to walk through, paths to trace, detail to
  reveal on hover. Ask for any visual, colour, or interaction that makes the plan easier to understand.
- **Emphasis** — the one or two things the reader must not miss: the riskiest decision, the accepted unknowns.
- **Test status** — a visible badge per test: `new`, `changed`, `existing`.
