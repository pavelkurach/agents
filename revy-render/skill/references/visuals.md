# Visuals

Judgement for choosing and drawing figures. Syntax, kinds, and colours live in [components.md](components.md).

Visuals, colour and interactivity: if it helps understanding, use it. Any figure, meaningful colour or interaction
that makes the content easier to understand is used, and nothing is held back for minimalism; the only test is
whether it helps understanding.

## Choose the form

The question for every piece of content: which form makes it easiest to understand? Mermaid and components are
both first-class; each wins where its shape fits:

- **Mermaid for anything with relationships, flow, sequence, state, or structure** — who calls whom, what happens
  next, what a thing turns into, what holds what. The engine does the layout, and the page adds zoom, full screen,
  tracing, legend filters and step-through (sequence message by message; flowchart and state diagram node by node,
  main path then failure paths), so one diagram can carry the whole system.
- **Components for shapes Mermaid cannot draw well**: a budget, a wire layout, a risk map, an HTTP contract,
  decision cards, dependency waves, a rollout rail, a grid, a file tree, before/after, examples. Their text reflows
  and cannot clip.
- **A free-form figure** only when neither draws the shape.

| Content shape | Form |
|---|---|
| Short linear chain, one verb per hop | Mermaid `flowchart LR`, a verb on each edge; `steps` when it is a one-line summary inside prose |
| Branching path with early exits and failures | Mermaid `flowchart TD`, every exit drawn |
| Pipeline or architecture with groups | Mermaid `flowchart LR` with `subgraph` for real boundaries |
| Calls across processes, services, transactions | Mermaid `sequenceDiagram`; `rect` for lock or transaction spans |
| Lifecycle, status, retry policy, protocol | Mermaid `stateDiagram-v2`, error states marked `failure` |
| Schema, entities, keys | Mermaid `erDiagram` |
| Types and their relations | Mermaid `classDiagram`, selected members only |
| What the user goes through, step by step | Mermaid `journey`, the user first |
| Containment or ownership | Mermaid `mindmap` or `treemap`; `flowchart` `subgraph` when edges matter |
| Dated change over time | Mermaid `timeline`; `gantt` for spans that overlap |
| Magnitude: trend, ranking, part of a whole, flow volume | Mermaid `xychart-beta` (line or bar), `pie` only for a real whole, `sankey-beta` when path volume is the point, `quadrantChart` for two scores |
| Tiers and who calls whom | `layers`, absent crossings struck |
| Slice or MR dependencies | `waves` |
| Files changed | `tree` |
| Before / after, option A vs B | `compare` |
| Config or capability grid, decision table | `matrix` |
| Rollout, migration, flag order | `timeline` |
| Latency or size budget | `budget` |
| Wire or memory layout | `layout` |
| HTTP contract | `endpoint` |
| Input → output cases, edge cases, rejections | `examples` |
| Measured run of operations | `trace` |
| Call tree | `stack` |
| Decisions with rejected alternatives | `cards` |
| Risks by likelihood and impact | `riskmap` |
| Real geometry, close-ups, small multiples, a notation nothing else has | `svg-figure` or `html-figure` |

## Craft

- **One claim per figure**, stated as its caption — the takeaway ("Cache hits skip Postgres"), not the topic.
- **Derived, never new**: every node, edge, and number traces to the document. A figure that drops an edge reads
  as complete: draw every exit or none, and note what a shape cannot carry.
- **Size follows clarity**: keep a large figure whole while it reads more easily than its parts; split into an
  overview plus detail figures when a reader can no longer follow one path through it.
- **Main path straight** — left to right or top to bottom; branches, stores, and side effects hang off the node
  that owns them, on one side.
- **Failure paths drawn**: every early exit with its condition on the edge, retry loops as real back-edges.
  A happy-path-only diagram is a defect.
- **Shape carries kind**: store as cylinder, decision as diamond, start and end as stadium, external dashed.
- **Group only for facts**: deployment, trust, ownership, runtime, persistence.
- **Labels**: real names (`RedisStorage::handle`, `POST /orders`), a noun phrase per node, a verb per edge,
  units on numbers (`9 ms`, `×3`). Wrap long labels with `<br/>`.
- **Colour whenever it carries meaning**: a kind class from components.md on what it describes — success path
  `ok` green, error paths `failure` red, additions `new`, alterations `changed`, removals `removed` — one accent
  for the path the figure is about, everything else quiet. A diagram arriving uncoloured gets its classes added
  when you restyle it. Unclassed edges and end nodes are coloured from their labels, so name outcomes plainly
  (`ok`, `timeout`, `exit 1`, `503`); class an edge yourself when its label would mislead.
- **Shape mirrors behaviour**: fan-out as a hub with radial edges, convergence as a funnel, a cycle returns to its
  start. Test: with all text removed, does the structure still hint at the idea?
- **Show the real thing** where a box would say "Response": a payload, an event name, a code line.
- **Name nodes by glossary terms or symbols** (`rules::for_languages`, `Coordinator`): the page makes those nodes
  open their definition or code. Mark the part a figure is about `focus` and the reader's side `user`.
- **One question per figure, revealed on demand**: the page adds tracing, legend filters, stepping and zoom by
  itself, so draw the whole system once instead of splitting it to show a path; split only when the overview stops
  being readable. A flowchart's step-through walks from its one entry along `ok` and unclassed edges before the
  failures, so give the diagram one entry, class the success path `ok`, and pin the order with `%% steps: A, B, C`
  when the story reads differently.

## Free-form figures

Prefer `html-figure` with CSS grid or flex — the browser measures text, so nothing clips. Use `svg-figure` for
real geometry, and then:

1. Sketch on a grid first; put every coordinate on it.
2. `viewBox` with at least 24 units of padding past every shape, stroke, arrowhead, and label end.
3. Size boxes from their text: width ≈ characters × 0.6 × font size (mono) + 2 × padding.
4. Text centred with `text-anchor="middle"`, baseline at centre + 0.35 × font size; extra lines as
   `<tspan x=… dy="1.2em">`.
5. Orthogonal edges ending on box edges, labels above the line with `.fig-halo`.
6. Draw order: boundaries, edges, nodes, labels. Opaque node fills.
7. More than a dozen shapes: generate the SVG from a small node and edge list with a script, then paste it.

## Mermaid pitfalls

- Quote labels holding `( ) : # % [ {`: `A["f(x): y"]`. A node id `end` breaks flowcharts.
- Layout follows declaration order: reorder, change direction, or split — invisible edges make it worse.
- Connect edges to nodes inside a `subgraph`, not to the subgraph.
- Raw colours ignore dark mode; use the kind classes.
