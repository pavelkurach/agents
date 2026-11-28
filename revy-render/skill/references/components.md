# Renderer blocks: schemas

Rule for every component: the reader never sees an id, slug, or generated code; things are named in plain English and cross-references (`needs`, crossings) use those names.

Every block below is a fenced code block in the markdown. The renderer warns with the field path and plan line
for anything it cannot use (`tree.files[2].status: "addded" is not one of added, modified, removed, context —
plan.md:41`) and draws the rest; a block it cannot draw at all is shown as source. Text fields accept inline
markdown: backticked paths link to the repository exactly as in prose.

Kinds (colour + a second signal, the same everywhere): `new` green `+`, `changed` blue `~`, `removed` red `−` and
struck, `failure` red `×` and heavier, `risk` orange `!` and dotted, `oneway` violet `⇥` and doubled, `accepted`
amber `✓` and dash-dotted, `context` grey `·` and dashed, `plain` neutral. Diagrams and the figure kit add `ok`
green with a `✓` disc: the success path, apart from `new` (added).

## Look file — `look.json`

```json
{ "order": ["Decisions", "Risks and accepted unknowns"], "fold": ["Orientation", "Impact"], "collapse": ["FAQ"],
  "emphasis": ["Risks and accepted unknowns", "exact text to highlight"], "tail": 5, "watermark": "DRAFT" }
```

Every field is optional. `order`: h2 titles shown first, in this order; the rest keep their place after them.
`collapse`: h2 titles whose `###` entries each become a closed disclosure, listed under the section in the side
navigation (an explanation's FAQ: `"collapse": ["FAQ"]`).
`fold`: h2 titles drawn closed, their summary a one-line digest (row/item counts, else the first sentence).
`emphasis`: an h2 title boxes its section; any other text highlights the block holding it. `tail`: a whole
number ≥ 1 — every list and table body longer than it shows that many and folds the rest behind "show N more"
(opened for print). `watermark`: a corner ribbon, or `null`. A title or text that matches nothing warns.

## In the markdown

Drawn without a block: status words as a table cell's first word or a bullet's first word (`added`,
`modified`, `removed`), the four markers, backticked paths (linked), footnotes (`[^1]`, listed at the end), and
a code fence's caption: text after the language (```` ```rust src/lib.rs — entry point ````). An image is never
embedded unless it is a `data:` URI; any other source becomes a link and warns.

The page is one HTML file, `<out>/<name>.html`, named after `--source` when given, else after `--plan`
(`plan.md` → `plan.html`); warnings and the header name that file too, with its line numbers. One small inline
script adds what follows; with JavaScript off every link still navigates, every tip is a native tooltip, and
nothing is hidden.

**Glossary.** A bullet list of `term: definition` items under a heading or a label containing "Glossary" (`##
Glossary`, or `Glossary:` inside any section) is a definition list. `a, b: definition` defines two terms. Every
occurrence of a term in prose, tables, components and Mermaid node labels gets a dotted underline: hover or focus
shows the definition, click goes to the section the definition links to (`[Stages](#3-stages)`), else to the
glossary entry. Matching is whole-word, case-insensitive, longest term first, and takes a plural `s`/`es`; never
inside headings, links, or a code span — a code span that is exactly a term (`` `state.json` ``) is the term.

**Evidence** (`--evidence <log.md>`). The lines under the log's `Evidence` heading, `E<n> via:<marker>
<citation> — <what it showed>` (optionally ending `| backs: <statement>`), are the ledger. An id `E3` in the
document becomes a chip: hover shows the citation and what it showed, click opens the folded **Evidence**
appendix after the last section at that entry, citation linked. Without `--evidence` ids stay plain text; an id
with no entry warns. A log without an `Evidence` heading fails the render.

**Code links.** A backticked path links to the repository host at the `--repo`'s pushed commit; `path:L245`,
`path:L245-L260` (also `path:245-260`, `path#L245`) and `path::symbol` link the lines (GitHub `#L245-L260`,
GitLab `#L245-260`, Bitbucket `#lines-245:260`). A module path `rules::for_languages` links like
`src/rules.rs::for_languages` when the evidence cites that symbol or the repo has one file named after the module.
`--link <web-url>@<sha>` points every link of the primary repo at that forge commit instead; then a file absent at
the sha is `local only` (plain, marked, and opening the evidence entry citing it when there is one), and a file
whose working tree differs from the sha — or a symbol defined only in the working tree — is linked and marked
`local differs`. Without `--repo`, `--link` links path-shaped spans unverified.

**Snippets.** A fence whose first word after the language is a path with lines, ```` ```rust src/rules.rs:240-260
E7 ````, numbers its lines from 240 and cites the path as a link to those lines; the words after it are the caption
(inline markdown, so `E7` is a chip). A list or paragraph right after the block whose every item or line starts
`L245 — note` (also `L245-L248`, `L63-L66, L69 —`) is its notes: each note links its first line, and hovering it
lights the lines it names. A note naming a line outside the snippet warns. Notes after a block without a location
number it from 1.

**Code chunks.** A note keyed by one range, `L244-L250 — why the guard comes first`, makes those lines a chunk:
tinted at rest, focusable, and on hover or focus lit as one with its note popped up beside it (links, terms and
chips in the note stay live). Note only the chunks that carry a decision, a non-obvious why, a contract or a trap;
every other line stays plain code. Touching chunks alternate tint. Warns: a range that runs backwards (`L250-L244`),
a range overlapping an earlier chunk (the first keeps the chunk; the later one stays a note), and, as for any note, a
line outside the snippet. Single-line and several-range notes (`L245`, `L240-L241, L250`) stay plain notes.

**Located diffs.** A ```` ```diff src/rules.rs E7 ```` fence whose body is `@@` hunks (a `git diff`'s file header
lines may stay) is drawn as the change: each hunk header a row, every line with its base and head numbers and its
sign, added and removed lines tinted, code highlighted in the language of the path's extension. Several hunks per
block are fine. The path links to the file at the link base; `PR #12` right after the path links it to the PR's
files view instead (GitHub `/pull/12/files`, GitLab `/-/merge_requests/12/diffs`, Bitbucket
`/pull-requests/12/diff`). Notes work as on snippets, keyed by head lines (`L44-L50 — `) or, with a leading minus, by
base lines (`-L40-L41 — `, for what was removed); head and base notes mix freely, and each range note is a chunk.
Warns: a note naming lines its side's hunks lack, a range spanning two hunks makes no chunk, a malformed `@@` header
(the block is then a plain diff), and a hunk whose line counts disagree with its header.

**Comma runs.** Three or more comma-joined items that are each a path or code span, optionally with a parenthetical
(`` `src/a.rs` (`parse`), `src/b.rs` (usage), `src/c.rs` ``), are shown one per line under the text before them, every
character kept, and warn `comma-joined list of N items rendered as a list — write one item per line`. Write such a
list as a list. Plain prose commas, and runs whose items carry words after their code, stay inline.

**Tables.** Columns whose every filled cell is a short value (a path or code span, a status badge, a number, ids,
or one short word) keep to their content on one line; prose columns share the rest and never shrink under 12
characters, so a crowded table scrolls inside its frame. With the page script, each header has a resize handle on its
right edge: drag it, focus it and press ←/→, or double-click it to fit the column to its content on one line; a
resized table shows "Reset widths" and remembers its widths in the reader's browser. `check.mjs` reports a prose cell
under 12 characters at 1440 and 1024.

**JSON.** An inline code span holding a JSON object or array (`` `{ "ok": true }` ``) and a component's code value
(an endpoint body `example`) are highlighted as JSON when they parse, or when their content type says JSON.

**Markdown places.** In a `.md` file a span's symbol names a heading (`fixer.md::Walk` opens the first heading
starting "Walk"), `::frontmatter` the front matter, `::L8` a line; a name followed by written lines
(`SKILL.md::Tier table:L18-L22`) labels those lines and links them. A tree whose `root:` is the repository's own name
finds its files inside the repository.

**The ledger.** `--evidence` lines are linted by ledger rules: comma runs stay inline, a bare citation that is one
place is one span, and whatever the ledger itself is warned about is printed after the document's warnings as
`evidence: …` under a count.

**References.** Every reference is a link with a popup on hover or keyboard focus (one popup system; Esc closes):
- `[Flows](#flows)` to a section, FAQ entry, footnote or definition previews its target (title and first line) and
  opens any fold on the way. A figure is linked as `#fig-` plus the opening words of its caption
  (`#fig-retry-and-breaker` for "Retry and breaker live…"), and previews its caption and a small copy of the drawing.
  An `#id` that matches nothing on the page warns.
- `commit <sha>` (7–40 hex), `PR #<n>` and `issue #<n>` link to the forge of `--link`, or else of the first `--repo`'s
  origin (GitHub, GitLab, Bitbucket); the popup is what the evidence ledger says of the ref, or the bare ref and its
  host. Without a forge they stay text.
- Outside URLs open in a new tab (`rel="noopener"`); the popup names the domain and the link text.
- An id the document defines — a heading `### F3 — …`, or a paragraph, list item or table cell opening with `**F3**`
  — makes every bare `F3` elsewhere (not in headings) a link to the definition with its preview. Only defined ids
  link; defining one twice warns and keeps the first.

**Lines set apart.** A paragraph starting `Trust:` (bold or not) is the trust callout; keep it before the first
section so it stays on the first screen under the title. `Answer:` is the answer callout; `Edges:` (a figure's
per-edge evidence) is a figure note.

**Navigation and theme.** The side navigation lists every section, each `collapse` entry, and the evidence
appendix; the current section is marked as you scroll. Below 1180 px the list sits under the header, and a compact
bar with the current section and a section menu appears once it scrolls away. A theme toggle follows the system
until the reader picks a theme, which is remembered; code, diagrams and every colour switch with it.

**Figures.** Hovering or focusing a flowchart node or a state lights its edges and neighbours; hovering an edge or
its label lights it and the two nodes it joins. A legend entry is a toggle that shows only that kind (press again or
Escape for all). Every Mermaid diagram and `svg-figure` has a tools row above its top-right corner, always shown:
zoom (−, the level, +, Fit) and Full screen. Zoom also follows Ctrl/⌘ + wheel or a pinch at the pointer and the
`+` `−` `0` keys while the tools have focus; a zoomed diagram drags to pan and double-click returns it to its size
at rest. Zoom redraws the SVG at the new size, so text stays sharp and node links and tips work at every level. Full
screen opens the figure (caption, tools, legend) over the page, fitted to the screen; Escape or the button closes it.
Step through (◂ ▸ or ←/→, Play; All or Escape ends it) walks a sequence diagram message by message, and a flowchart
or state diagram node by node — the main path, then each failure path, then side inputs — lighting the current node
and edge, keeping the path so far, dimming the rest, and showing the edge's label as the caption (red on a failure
edge, green on an `ok` one). The current step stays in view when zoomed and in full screen. Without the script the
diagrams draw at their size and no tools show. Any box that scrolls sideways (a diagram frame, a wide table) fades
at the edge that hides content and shows a "scroll →" cue in its bottom-right corner until its end is reached.

**Lint.** `render.mjs --lint --plan <file> [--source …] [--look …] [--evidence …] [--repo …] [--link …]` parses
and draws everything as a render would, prints the same `warn:` lines and an `error:` line for each block shown as
source (a Mermaid or component block that cannot be drawn), writes nothing, and exits 1 when there is an error.

## Mermaid — ```` ```mermaid <caption> ````

Rendered to inline SVG at build time (Pele). Text after `mermaid` on the fence line is the caption: one sentence,
the figure's claim; it is also the SVG's `aria-label` (else the first node's label).

- Every Mermaid 12.1 diagram type renders (flowchart, sequence, state, class, ER, journey, timeline, gantt, mindmap,
  treemap, xychart, pie, sankey, quadrant, …).
- Nodes: `A[Parse]:::new`, or `class A,B failure` (state diagrams: `class S failure`; ER: `ORDER:::new {`).
- Edges: Pele puts no class on an edge. Give the edge an id and class it: `B e1@-->|no| E` then `class e1 failure`.
- Sequence: a message with a cross head (`-x`, `--x`) is drawn as a failure.
- Semantic classes: `new` `changed` `removed` `ok` `risk` `failure` `oneway` `accepted` `context`, `focus` (the
  part the figure is about: highlighter fill, heavy border) and `user` (the reader's side, the accent). `ok` is the
  success path: green, heavier, and a `✓` disc on the node's corner or before the edge's label. A legend naming the
  ones used is added under the figure.
- Outcome colours by default: an edge (any diagram type) or a flowchart end node (no outgoing edge) left
  unclassed takes `failure` or `ok` from its label; any author class wins, and inferred ones also carry the class
  `inferred`. Whole words, case-insensitive, identifiers split at case changes (`ShellError` names `error`).
  `failure`: err, error, fail, failed, failure, reject, rejected, invalid, denied, timeout, panic, abort, `exit`
  with a non-zero code, a leading 4xx/5xx status (`503 Retry-After`, `4xx`), and a flowchart cross head
  (`A --x B`). `ok`: ok, success, succeeded, pass, passed, valid, accepted, done, `exit 0`, a leading 2xx status.
  A status number mid-sentence (`retry after 200 ms`) is not one; a label naming both outcomes stays neutral.
- Sequence: `rect focus … end` marks the span a figure is about; `rect rgb(…)` still renders but warns. A `;` in
  message text is kept (Mermaid would end the statement there).
- Nodes and participants whose label is a glossary term, a symbol or a path are links with a tip (term:
  definition; symbol: `path:L<n>` and the evidence citing it), exactly like text. Label nodes by those names.
- `accTitle:` and `accDescr:` become the SVG's accessible title and description.
- Step-through order (flowchart, state): the walk starts at the node no edge enters (a state diagram's top-level
  `[*]`), follows `ok` then unclassed edges to a node already walked or a dead end, then walks each failure edge
  (classed or inferred `failure`, or entering a `failure` node) in source order, then what is left. When that order
  misleads, pin it with a comment listing node ids: `%% steps: A, B, C, F`. Each node is reached by the edge from the
  node before it; a node not reached from it starts a new path from the latest earlier node with an edge to it. An id
  that names no node warns with the diagram's node ids and is skipped; `%% steps` on other diagram types warns.
- Code in a label — a backticked span, or code-shaped text like `s3::put_object` or `with_retry` — is drawn
  mono on a grey pill, like inline code in prose (only the code part of a mixed label); a quoted label is left as
  written. Node labels share one size, edge labels another.
- `classDef`/`style`/`linkStyle` with a raw colour still render but warn (they ignore dark mode); the warning names
  the class to use.
- Too wide for the column: a flowchart (or other LR diagram) is re-laid out first — long labels wrapped and ranks
  closed up, then LR turned TD — keeping its links, tips and tools. What still does not fit scrolls in its own box
  behind the scroll cue; text never drops below 12 px. Phones get a narrower layout when Pele can make one.

## Free-form figures — ```` ```svg-figure <caption> ```` / ```` ```html-figure <caption> ````

The body is emitted verbatim. Draw with the kit so the figure follows both themes:

- Tokens: `var(--ink)` `--ink-2` `--muted` `--bg` `--surface` `--surface-2` `--line` `--line-strong` `--accent`,
  and per kind `--new` `--changed` `--removed` `--ok` `--failure` `--risk` `--oneway` `--accepted` `--context`
  (each with `-bg`).
- Classes: `.fig-node` (box), `.fig-boundary` (dashed group), `.fig-edge` (+ `.is-async` dashed, `.is-hot` accent),
  `.fig-label` (+ `.is-mono`, `.is-muted`, `.fig-halo` for text crossing a line), `.fig-fill-hatch`; add a kind
  class (`k-new`, `k-ok`, `k-failure`, …) to colour any of them.
- Markers: `marker-end="url(#fig-arrow)"`, `#fig-arrow-open`, `#fig-dot`, `#fig-cross`; they take the edge's colour.
- Rules: every `<svg>` has `viewBox` and `aria-label`; raw colours (`#hex`, `rgb()`), `<script>`, and anything
  loaded from the network (`src`/`href` to `http(s)://`, `url(http…)`, `@import`) warn.

## Components — the body is YAML

`caption:` (optional, every component) is the figure's claim, shown above it.

### `tree` — files changed
```yaml
root: revy-review/cli          # optional prefix for every path
files:                         # required, ≥1
  - path: src/json.rs          # required; directories become group heads
    status: added              # required: added | modified | removed | context
    note: pure JSON encoding   # optional, one clause
    delta: +80 -0              # optional
```

### `timeline` — rollout / migration rail
```yaml
steps:
  - title: Ship behind the flag   # required
    when: wave 1                  # optional; an order (wave N), never a duration or date the document lacks
    kind: step                    # step | gate | oneway (default step); the rail turns violet after a oneway
    detail: flag off everywhere   # optional
    rollback: delete the flag     # optional; a oneway step without one says "no rollback"
    current: true                 # optional; marks where the rollout is now
```

### `budget` — latency / size budget
```yaml
unit: ms         # required
limit: 200       # required, ≥ 0
parts:           # drawn as a waterfall against the limit; the part past it is hatched red
  - label: parse
    value: 12    # required, ≥ 0
    kind: plain  # plain | new | risk | changed
    note: …      # optional
```

### `layout` — wire / memory layout to scale
```yaml
unit: bytes      # optional (default bytes)
fields:
  - name: magic
    size: 4      # required, ≥ 0
    kind: plain  # plain | new | changed | removed | reserved
    note: …
```

### `compare` — before / after
```yaml
before: Today          # column titles (defaults Before / After)
after: With --json
rows:
  - label: stdout      # optional row head
    before: text lines # a row with only before is removed, only after is added; equal sides read "unchanged"
    after: one JSON document
```

### `matrix` — grid with tinted cells
```yaml
corner: Command        # optional top-left label
columns: [text mode, json mode]
rows:
  - label: success
    cells:             # one per column; text, or {text, kind}
      - "`file: ok` lines"
      - { text: "`{ok:true}`", kind: new }   # plain | same | differs | new | removed | risk | failure | context
```

### `riskmap` — likelihood × impact
Each grid cell shows the risk's title (wrapping as needed); no ids are generated.
```yaml
risks:
  - title: unknown `at` prefix   # required
    likelihood: low              # low | medium | high
    impact: medium               # low | medium | high
    note: …                      # optional; listed under the grid by title
```

### `stack` — call tree
```yaml
frames:
  - call: main::parse      # required; a repository path (`src/a.rs::parse`) links like a code span
    mark: called           # called | added | removed | changed | context
    note: one clause       # optional
    calls: [ …frames ]     # optional, nested
```

### `layers` — tiers, nodes, crossings
```yaml
tiers:
  - name: CLI
    note: optional
    nodes: [main.rs, { name: json.rs, kind: new }]   # text, or {name, kind}
crossings:                 # optional; from/to name nodes
  - from: main.rs
    to: json.rs
    label: calls
    absent: false          # true = this edge must not exist (struck, ×)
    note: …
```

### `waves` — dependency waves of slices / MRs
```yaml
items:
  - title: Add the --json flag   # required, unique; it is the item's name everywhere
    needs: []              # titles of other items, exactly as written there; waves are computed from these (unknown title or cycle warns)
    status: backward-compatible   # optional; a status word becomes its badge
    note: …
```

### `cards` — decisions
```yaml
decisions:
  - question: What does one error entry contain?   # required
    chosen: "`{file, at, message}`"                # required
    reversibility: one-way                         # one-way | reversible | internal
    why: …                                         # optional
    cost: …                                        # optional
    owner: user                                    # optional: user | planner, shown as a pill
    rejected:                                      # text, or {option, reason}
      - option: "`{file, message}`"
        reason: the subagent loses where to fix
```

### `steps` — short linear chain, one verb per arrow
```yaml
steps:                      # ≥ 2; text, or {node, verb, kind}
  - { node: parse args, verb: validates }
  - { node: run check, verb: prints }
  - { node: JSON on stdout, kind: new }
```

### `endpoint` — HTTP contract
```yaml
method: PUT                       # GET | POST | PUT | PATCH | DELETE | HEAD | OPTIONS
path: /blobs/{id}                 # required
summary: one sentence             # optional
auth: { scheme: bearer token }    # exactly one: { scheme: … } or { open: why it needs none }
params:                           # optional
  - { name: id, in: path, type: uuid, required: true, note: … }   # in: path | query | header | body
request: { type: application/json, example: '{ "a": 1 }' }       # optional
responses:                        # ≥ 1; no 4xx or 5xx warns
  - status: 503                   # 100-599; 2xx green, 3xx grey, 4xx orange, 5xx red
    meaning: breaker open
    kind: new                     # optional: new | changed | removed
    headers: [Retry-After]        # optional
    type: application/json        # optional
    example: '{ "error": "BreakerOpen" }'   # optional
```

### `examples` — given / when / then
```yaml
rows:                             # one row per distinct behavior
  - given: breaker open           # optional; shown as — when absent
    when: "`PUT /blobs`"          # required
    then: 503 with Retry-After    # required
    kind: error                   # wire (default) | boundary (orange) | error (red)
```

### `trace` — operations in lanes
```yaml
ops:                              # in order; one lane per distinct key, first seen first
  - { op: W1, key: page-4 }       # op and key required
  - { op: R1, key: page-9, kind: failure }   # kind: as steps (plain | new | changed | removed | failure | risk | oneway | accepted | context)
legend:                           # optional: what each op symbol means
  - { symbol: W, meaning: write }
```
