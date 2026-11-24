# Presentation

The renderer owns the output. `revy-plan render` turns the plan into Markdown and a standalone HTML
artifact; `revy-plan serve` serves it at one loopback URL. Generation runs one way — nothing is read
back out of the rendered file, and you never write that output by hand. This file is the judgement the
renderer cannot supply: **which shape fits which content**, so the structured plan carries the right
one, authored as typed content rather than markdown.

## Two shapes carry everything that is not table or prose

A `Section` is a heading plus a sequence of blocks. Two block kinds hold anything a picture cannot:

| Block | Carries |
|---|---|
| `Code` | one syntax-highlighted excerpt — `lang`, an optional `caption`, `code` |
| `Figure` | one `Diagram` — a closed kind, picked from the table below |

Everything else in this file is which `Diagram` kind to reach for and its authored fields. `orientation`
is text, not blocks — `placement`, `touched_flows`, `glossary` — see [behavior.md](behavior.md); a
`Section`'s blocks are where a drawing actually lands.

> **If it can be shown, show it** — but a `Figure` carries a *shape*, not sentences. Prefer a `Figure`
> to a paragraph wherever one of the fifteen kinds fits.

## How much text fits

The renderer draws into fixed geometry; prose stuffed into it clips or overlaps. Keep authored strings
to the length their slot draws:

- **`brief.title` is the page heading** — a short noun phrase, ≤ 60 characters, no colon-and-clauses.
  It is the plan's name ("Add the four remaining Redis list commands"), not a sentence. Always author it;
  without it the renderer truncates `summary` into an ugly mid-sentence "…".
- **`brief.summary` is the lead paragraph** under the heading — a few sentences framing the change. Not a
  title, not the whole design; the detail belongs in `scope`, `current_behavior`/`target_behavior`, or an
  authored `Section`.
- **Backtick every code identifier and file path in prose** — `` `RedisStorage::handle_commands` ``,
  `` `src/server.rs` ``. Authored prose renders as Markdown, so a backticked span becomes a `<code>` pill;
  an un-backticked identifier is indistinguishable from ordinary words.
- Inside a `Figure`, a **`label` is a noun phrase ≤ 40 characters** and a **`note` is a short clause
  ≤ 80**. If a field needs a full sentence, it is prose — a `Prose` block or a table cell — not a diagram.
- An MR's **`standalone_behavior`** is a sentence or two on what the slice delivers on its own, not its
  whole design; the full detail is the plan around it, not one table cell.
- **A `stack` or code excerpt is a call-*flow*, not a source paste.** Show the function/method names and
  one short clause on what each does or changes; never paste `match` arms, full bodies, or every
  disambiguation comment. If a step needs a paragraph of code to explain, it is prose, not a frame.

## The fifteen kinds

`kind` is the tag every `Diagram` carries. Reach for the one matching the *shape of the thing*, not the
section you happen to be in — a plan that reaches for the same three kinds everywhere has stopped
looking. Fields marked `?` are optional; every other field is required. `layers.edges` and `dag.edges`
share one `Crossing` shape: `{from, to, label?, note?, absent}` — `absent: true` draws the row struck
and unarrowed, the one way to state "this edge does not exist" without omitting it.

| `kind` | Use it for | Fields |
|---|---|---|
| `layers` | Tiers and their components, and the crossings that matter | `layers[].{name, nodes[].{name, kind?: new\|external}}`; `edges[]` (`Crossing`, above) |
| `flow` | A pipeline (horizontal), or a path with every early exit drawn (vertical) | `flow.orientation: horizontal\|vertical`; horizontal → `steps[].{name, note?}`; vertical → `steps[].{name, note?, terminal, outcomes[].{when, then, kind: error\|exit}}` — `kind` is required on every outcome, never defaulted |
| `seq` | Anything crossing a process, service or transaction boundary | `actors[].{name, note?}`; `steps[].{from, to, label, kind?: return\|self\|error, note?}` — `from`/`to` name a declared actor; `spans[].{from, to, label, kind?: tx\|lock}` — `from`/`to` are the actor's 0-based position in `actors` |
| `er` | Entities, their fields and their keys | `entities[].{name, kind?: new\|context, fields[].{name, type, key?}}` — a field's `key` is `{kind: primary}` or `{kind: foreign, references}`, never both; `relations[].{from, to, kind: one_to_many\|many_to_one\|one_to_one, label?}` |
| `endpoint` | One HTTP contract, every status | `method: get\|post\|put\|patch\|delete, path, summary?, auth` (required — `{kind: open, reason}` or `{kind: scheme, name}`, so "no auth" is always stated, never just absent); `params[].{name, in: path\|query\|header\|body, type, required, description?}`; `request?: {content_type?, example?}`; `responses[].{status, description, kind?: new\|changed, headers[], body}` |
| `dag` | Rollout/MR waves, authored by hand — distinct from the CLI's own derived MR graph | `caption?`; `waves[].{name?, mrs[].{id (slug), title, kind?: migration\|oneway\|scaffolding, flag?, run?, note?}, gate?}`; `edges[]` (`Crossing`, above) |
| `states` | A lifecycle, protocol or status column | `states[].{name, kind?: initial\|terminal\|error\|new, note?}`; `transitions[].{from, to, on?, kind?: error\|new, note?}` — `from`/`to` are plain names, not cross-checked against `states` |
| `layout` | A wire or memory representation, drawn to scale | `caption?, unit`; `fields[].{name, size (>0), kind?: new\|reserved\|removed, note?}` |
| `timeline` | Rollout, migration and flag ordering | `steps[].{name, kind?: gate\|oneway, wait?, rollback?, note?}` |
| `tree` | A file or config hierarchy, with what changes | `entries[].{path, change?: added\|modified\|removed, note?}` — every entry names a concrete file, never a bare directory row |
| `matrix` | A config or capability grid | `columns[]`; `rows[].{name, cells[].{column, value, kind?: differs\|new\|risk}}` — `column` must name a declared column; a cell left out for a declared column reads as context |
| `trace` | A measured run of operations, one lane per thing acted on | `caption?, legend[].{symbol, meaning}`; `ops[].{op, key, kind?: waste\|saved\|new}` — `key` groups ops into lanes, first-seen order |
| `budget` | A latency or size budget split across the hops that spend it | `budget, unit`; `parts[].{name, value, kind?: new\|risk}` |
| `compare` | Before and after | `columns[].{label, items[].{text, kind?: add\|del}}` |
| `stack` | A call tree | `frames[].{marker?: added\|removed\|changed\|called, symbol, note?, children[]}` — `children` recurses, one frame per level |
| `examples` | Concrete input→output pairs — the wire contract, edge cases, rejections | `caption?`; `rows[].{given?, when, then, kind?: wire\|boundary\|error}` — `when`/`then` required; `given` only when the result depends on prior state; renders as a `given / when / then` table, rows tinted by kind |

**This is a floor, not a menu.** A config change is a `compare`, a lock ordering is a `seq` with threads
as actors, a packet format is a `layout`, a retry policy is a `states`. A behavior stated as concrete
inputs and their results — `LPUSH k a b c` → `c,b,a`, a missing key returning null, a rejected arity —
is an `examples`, never a comma-pile sentence: put the input→output pairs in the table, not the prose.

## Never author these — the renderer derives them

Ten figures the renderer emits straight from the plan's typed data, whenever that data exists and never
omitted when it does. **Do not author any of them as a `Figure`** — a hand-drawn copy beside the CLI's
own is a second source that drifts. The fifteen kinds above are yours to author; these are the render's.

| Figure | Derived from |
|---|---|
| Coverage strip | `coverage` |
| Impact matrix — subject × state × shape | `impact` |
| Evidence → claim map, with a `sampled` marker per evidence item | `claims`, `evidence` |
| Changed-symbol tree — files nested by directory, symbols under files, change kind coloured | `changed_symbols`, slice `files` |
| As-is call graph | `SemanticObservation::CallEdges` |
| MR-DAG waves | `DeliveryGraph::waves()` |
| AC ↔ MR ownership, with each slice's `verify` | `delivery` |
| Decisions, with `ruled_out` and owner | `candidate.decisions` |
| Review timeline — one lane per wave, findings coloured by severity, triage/ruling per finding, lane verdicts, consumer checks | `history` |
| Unresolved facts | `uncertainties` |

## Two rules hold whatever is drawn

Every `Figure` is **derived from data the plan already states elsewhere** — a diagram authored beside
its table is a second copy that drifts. And **a diagram that silently omits an edge is worse than the
prose it replaced**, because it reads as complete: draw every exit or none, and note any dependency a
kind's shape cannot carry (`dag`'s `edges` list, or a `note` field) rather than dropping it.

**Colour is a category axis, never emphasis** — the same colour means the same kind of thing everywhere:
green passes or is added, red fails or is removed, amber is a key or warning, violet is new or one-way,
grey is context.

Prose stays for what a picture cannot carry: why a decision beat its alternative, what a cost is, what
would falsify a claim. Run `revy-plan schema` for the exact structure the renderer accepts; a shape that
fails to validate is a visible error at `apply`, never a silent gap in the render.
