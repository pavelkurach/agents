# Writing

## explanation.md

Content only — diagrams as Mermaid, since their nodes and edges are claims the verifier checks. How the
markdown becomes the page is § Presentation.

Sections, in order:

1. **Title + Answer** — a noun-phrase title (≤ 60 chars), the request verbatim, then `Answer:` and the answer in
   one or two sentences. A reader who stops here leaves with the gist. Then the **trust line**, from the last verify:
   `Trust: N claims · V verified · C corrected by verification · G open`, the open count linking to Gaps. Count
   the final explanation: N its claims, C those fixed in any round, G its Gaps entries, V the rest.
2. **Map** — one C4 context diagram: the system, its neighbours, the user, and where the subject sits among
   them. Draw it `flowchart TD`, or with at most about four nodes per row, so it fits the page width.
3. **First principles** — the mental model, built from the ground up before any detail:
   - the problem the subject solves, stated without the system's own vocabulary;
   - the forces any solution must answer — constraints, failure modes, scale, who can be trusted;
   - the core concepts, each introduced only when the problem needs it, with the one thing it exists to do;
     a general concept the subject rests on (a merge-base, an index, a lease) is explained from scratch too;
   - the design as a consequence: a diagram from forces to concepts to mechanisms, so the reader could
     re-derive it.
   Claims about this system cite evidence; a general concept cites a `via:doc` source when it is not common
   knowledge.
4. **Glossary** — every term the cold reader would look up — domain words, internal names, acronyms — as
   `- term: definition` bullets: its one-line meaning here and, when a section or diagram explains it in full, a
   markdown link to it.
5. **Body** — the mode's Body sections, each detail hung on the model from First principles.
6. **Owner's view** — the SKILL.md § Owner's depth rows: why it is this way, what it guarantees and who enforces it,
   its limits, its intricacies, its trade-offs — each a table or diagram, each row cited.
7. **Gaps** — each claim evidence could not settle, with what would settle it; `none` when there are none.
8. **Separately** — issues noticed outside the subject's scope, one line each, so the body stays on its subject;
   left out when there are none.
9. **Start here** — the one first step to take with this knowledge, doable in minutes: a file to open, a
   command to run, a question to settle with someone.
10. **FAQ** — one `###` entry per follow-up, oldest first: the question verbatim as the heading, the answer shaped by
   these same rules, and what it changed in the body, if anything.

## Rules

- **Picture first.** Shape carries the meaning: a diagram for anything with steps, branches, layers, states, or
  structure; a table for things that share fields; a list for anything listable. A flow of three or more hops,
  any branch, a lifecycle, or a layering is a Mermaid diagram, failure edges included. The diagram or table opens
  its section; its caption states the fact the section turns on. Prose only for a reason or a trade-off.
- **Zoom like C4.** Each structure diagram stays at one level and its caption names it: *context* (the system,
  its users, outside systems), *container* (each separately run or stored piece — process, CLI, database,
  queue, file store — and the channel between them), *component* (the parts inside one container), *code*
  (types and functions). Zoom in one level at a time, from the Map down to the subject; a level with nothing new
  to show merges into the next. Draw them as flowcharts — Mermaid's own C4 syntax lays out poorly.
- One diagram, one question, and the question picks the form: who calls whom, in what order → sequence; what
  states, and what moves between them → state; what depends on or contains what → flowchart; what the user goes
  through → journey; how variants differ → one small diagram per case, same layout. Past about 12 nodes, draw an
  overview plus one diagram per stage.
- Every edge label is a verb naming the mechanism: *calls `refresh()`*, *writes `state.json`*, *302 to
  `/login`*.
- Mark meaning with the renderer's classes, the same names everywhere: `user`, `ok` (a success path), `failure`, `focus` (the part a
  task or finding touches), and for a change under review `new`, `changed`, `removed`. In sequence diagrams
  `focus` is a `rect focus` block and a failure an `alt` branch.
- In sequence and journey diagrams, the user is the first participant.
- One name per thing across diagrams, tables, and text; each is a Glossary term or a cited symbol, so the page can
  make it interactive.
- Two or more items are a list; three or more items sharing fields are a table; a cell is a clause.
- A sentence states one thing in under 20 words. A paragraph is at most two sentences.
- Every line stands alone — a popup, a note, a table row read by itself still makes sense. Name the thing again
  rather than pointing back to it.
- A section's caption — or its first line, when no picture opens it — carries its point: a reader who reads only
  those still gets the explanation.
- Literal words. A hedge appears only where something is genuinely uncertain, and says what.
- A problem is stated as its cause and its effect.
- Name each case concretely: *expired token → `401`, redirect to `/login`*.
- Show the user's side wherever there is one: what they see, click, wait for, and get on failure.
- **Show the code a claim turns on** — the branch that decides a flow, the type that shapes data, the contract a
  caller relies on, the line a finding cites. Each snippet:
  - is copied verbatim from the explained revision, at most about 20 lines, starting and ending at a syntactic
    boundary — a whole statement, block, or item — with elisions marked `…`;
  - is fenced with its language, location, and evidence id on the fence line itself —
    ```` ```rust src/rules.rs:240-260 E7 ```` — lines of the explained revision;
  - sits beside the claim or diagram stage it backs;
  - is followed by notes keyed by cohesive chunk — a guard, a loop, a transformation, a call and its error
    handling — `L250-L258 — retries once, then gives up`, or a single line written as a range (`L247-L247`) when one line is the point, only where
    the chunk carries a decision, a non-obvious why, a contract, or a trap. Note lines follow the closing fence
    directly.
- Every claim about code or behaviour cites its evidence id: *the session lives 24 h (E4)*. A diagram cites per
  edge — in the edge label, or an `Edges:` line beneath it (`Edges: A→B E3 · B→C E3, E4`) — or once in its
  caption when one id backs every edge.
- **Every reference is a link**, so the page can make it clickable and hoverable:
  - a section or FAQ entry as a markdown link to its heading — `[Flows](#flows)`; a diagram by its caption —
    `[the container view](#fig-containers-of-the-fix-loop)`;
  - code as `path:L245-L260` or `path::symbol`;
  - evidence by id (`E7`), a finding by its report id;
  - history as `commit <sha>`, `PR #<n>`, `issue #<n>`;
  - an outside document by its URL.
- Backtick every identifier, path, and command.
- Say each thing once.

## Presentation

The conventions in this file follow `revy-render`, the recommended renderer. How each becomes the page — glossary
popups, evidence chips and the folded appendix, code links, snippet line numbers, classes, themes, the side nav —
is defined once, in `~/.claude/skills/revy-render/references/components.md` § In the markdown. Follow it; this
file never restates it.

Lint before rendering: `node ~/.claude/skills/revy-render/render/render.mjs --lint --plan explanation.md --evidence log.md`.

Ask the renderer for what is specific to an explanation:
- the sections in the order written;
- Title, Answer, the `Trust:` line, and Map on the first screen;
- FAQ collapsed per entry (`"collapse": ["FAQ"]`).
