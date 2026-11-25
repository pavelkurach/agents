# revy-review — spec

Design notes behind the rules base: `notes/`.

## Principles
1. **Floor, never ceiling.** Prompts set the minimum a review must cover and never cap the agent: lenses add depth,
   never narrow scope; no lead or step limits, no named model or harness tool. Better models and harnesses improve the tool for free.
2. **Owner-curated rules.** The rules floor is the owner's knowledge base (tave format: topic folders, one rule per
   file with Principle / Why / Flag when / Skip when, language packs, cross-file patterns).
3. **Understandable and maintainable by the owner.** Prompts in Markdown; code only where determinism matters
   (schemas, quotes at head, report round-trip, comment text). Rust only.
4. **Any harness.** A skill in the open Agent Skills format; dispatch uses whatever subagent mechanism the harness has.
   No code ever calls a harness or a forge.
5. **Impact beyond the diff is the core.** Every reviewer traces every touched contract outward to all callers and
   consumers and every new assumption inward to all producers, with the best code intelligence available.

## Rules
- Two rules folders, in load order: the skill's `rules/` (required) and the owner's `~/.revy-review/rules/`
  (optional). In each, every subfolder is a topic, one rule per file `<topic>/<id>.md`, the file name is the id
  (unique across the topics of both folders). Format in `rules/README.md`: optional `langs` hint and `When` (read by the helper),
  `Ask` / `Report as` and optional `Not when` / `Example` (read by the reviewer). Re-authored; nothing copied from
  tave. No rule topic is known to the code.
- Custom rules: the owner adds files to `~/.revy-review/rules/<topic>/`, a new folder for a new topic. No code
  change.
- `revy-review rules` validates both folders (a `When:` line in every file, unique ids; errors name the
  file, a duplicate id names both files) and prints one index the reviewers read: per folder a `Rule files:` header,
  then one line per rule (its `langs` is a free-text hint reviewers read; no extension filtering). Reviewers open only rule files whose `When` matches the diff; `When: always`
  asks them to apply the rule to every changed file.
- Language focus: Python and Rust first (packs, fixtures, evals); Go and TypeScript packs exist but are secondary.

## Invocation
`/revy-review [target] [fix]`
- no target: current branch vs its merge-base with the default branch; uncommitted work is snapshotted into a
  commit (`git stash create`) and that sha is the head
- `<PR/MR number or URL>`: the PR head vs its merge-base
- `<commit>`: `commit^..commit`
- `<A>..<B>`: only that range (incremental re-review); impact still traced across the whole repository
- `fix` (self mode): fix every finding, re-review the fixes, repeat until a round finds nothing new (max 3 rounds),
  one commit per round, never push. Questions are settled first: fixed if confirmed, else answered in one line.
  Exclusions: "fix all except 4" or `post: no` in report.md.

The agent resolves the target and records `base`/`head` in the review state; every later step uses them.

## Steps (SKILL.md)
0. Problem gate — one agent (`problem.md`). Sources: ticket, PR/MR description, linked issue, commit messages; self mode
   with none of them → ask the owner for a one-line goal; team mode → infer from commits and mark it inferred.
   Writes `problem.json`: statement, acceptance criteria (each quoted from its source, or marked implied), out of scope,
   and per criterion `met | partly | not met | unverifiable` with `path:line` evidence; unrelated changes = scope creep.
   **Gate:** if the change does not address the problem, or a core criterion is `not met`, stop here: report only the
   gate findings (self mode: offer to fix first; team mode: the report says so and asks whether to run the deep review
   anyway). `partly` and `unverifiable` do not block. Otherwise the criteria are passed to every reviewer as the
   scenarios that matter most, and the gate's findings join the reviewers' at the verifier.
1. Four lens reviewers in parallel (two lens pairs, each drawn twice), same full brief, plus a context step
   (ticket and links, git log/blame of touched code, project docs) (`reviewer.md`: impact outward/inward,
   how-to-look, rules floor, impact ledger, structured findings) → `reviews/<n>.json`; each must pass `revy-review check`.
   Lenses (under test, revy3p): each reviewer also gets one "review everything, but go deepest on X" line —
   impact · failure and concurrency · tests and edge inputs · design and simplicity.
   Plus one subagent per specialist (see Extensions), same output schema and validation.
2. One verifier (`verifier.md`): merge same-root-cause findings, walk each scenario in code, drop only when the code
   contradicts it (refuting only the stated reason is not enough — judge the fix on its merits), unresolved →
   question → `verified.json`; must pass `revy-review check`.
3. `revy-review sync` renders `report.md` (editable) from `verified.json`.
4. Self mode: deliver report, or run the `fix` loop. Team mode: curate → post.

## Extensions
- Rules: see Rules.
- Specialists: `reviewers/<name>.md` in the skill (the built-in focused reviewers) and in `~/.revy-review/` (the
  owner's), loaded in that order, file-name order within each; the file stem is the name, unique across both (a
  duplicate is an error naming both files). A file is optional header lines in any order — `rules: <topic>, …`
  and `dispatch: always | when relevant` (absent: `always`; any other value is an error naming the file) — then
  the prompt (the focus). An `always` specialist runs on every review; a `when relevant` one runs when the
  coordinator judges, from its prompt and the change, that the change has something for it, and when unsure it
  runs — no path or extension matching. The coordinator records every decision in `state.json` `specialists`.
  The dispatched specialists run as reviewer n = 5, 6, … in load order after the lens reviewers, with
  `reviewer.md` and its file in place of a lens; it reviews only what its focus covers, or writes an empty review
  when nothing falls under it. It receives the rules of its `rules:` topics from both rules folders, and no rules
  without the line; a topic no rules folder has is an error. A missing prompt is an error, and that file is not
  dispatched. Specialists are the one extension point for reviewers: start a concern as a rule; promote it to a
  specialist when it needs its own context (e.g. repo-wide duplication).

## Curation and posting (team mode)
- `report.md` is the editing surface: per finding `post: yes|no`, severity, title, body; a `## My findings` section
  takes `path:line — text`. The user edits it, or tells the agent ("drop 4", "make 6 a question, softer", "add: …").
- `revy-review sync` reads back what posting needs from an edited report.md, with exact errors (bad post flag,
  severity or site, a posted block without its fix) and a warning for a line outside the diff (it would post as a
  general comment).
- `revy-review comments` prints each post-ready comment: `path:line`, conventional-comments header
  (`issue (blocking): …`), why it breaks, fix. No process artifacts (ids, rule ids, paths, tool mentions). Optional
  footer is config, off by default. Tone follows the owner's `voice.md`.
- The agent posts them as a **draft/pending review** with whatever forge tool exists, after confirming the scope;
  verifies each landed inline where intended.

## Formats (contract between the skill and the helper)
All JSON, UTF-8. `site` = `"path:line"` (repo-relative, line at `head`). Ids are stable strings.
- `state.json`: `{"repo": abs path, "target": text, "base": sha, "head": sha, "mode": "self" | "team",
  "round": n, "skill": abs path, "specialists": {name: "dispatched" | {"skipped": reason}}}` — written by the
  agent; every command reads it from the review directory (`mode` and `round` steer only the skill).
  `specialists` is added before the reviewers are dispatched and is the one record of which specialists ran:
  `check` requires it once any review file exists (when specialists load), with exactly one decision per loaded
  specialist, no unknown name, a non-blank reason per skip, and no skip of an `always` specialist. `skill` is this skill's directory. The helper, not the agent, resolves the
  sources from it (`<skill>/rules`, `<skill>/reviewers`, `~/.revy-review/rules`, `~/.revy-review/reviewers`): the
  layout lives in one place, `rules` and `check` see the same sources, and an absent plug-in folder needs no
  bookkeeping in the agent.
- `problem.json`: `{"statement", "source": "ticket" | "description" | "commits" | "owner" | "inferred",
  "criteria": [{"id": "C1", "text", "quote": source text | null, "implied": bool, "verdict": "met" | "partly" |
  "not_met" | "unverifiable", "evidence": [site]}], "out_of_scope": [text], "scope_creep": [{"id": "S1", "sites": [site, ...], "why"}],
  "blocks": bool}` — `check` recomputes `blocks` (any core criterion `not_met`, or no criterion met) and
  rejects a mismatch. A `scope_creep` entry is one cause with every site it touches (`sites` non-empty); ids are
  `S<k>`, unique.
- `reviews/<n>.json`: `{"reviewer": n, "lens": text | null, "ledger": [{"kind": "contract" | "unit", "subject":
  contract text or unit name, "site", "verdict": "fine" | "finding", "finding": finding id | null, "cases": {dimension: text, …} (unit only)}],
  "findings": [{"id": "<n>.<k>", "title", "severity": "blocking" | "non-blocking" | "question" | "suggestion",
  "sites": [site, …], "quote": exact line at the first site, "breaks", "fix", "rule": rule id | null}]}`.
  A `unit` entry must carry `cases` answering every floor
  dimension (size, boundaries, absence, shape, variants, state, failure, concurrency, vs_base) with a non-empty text (the case
  walked and its outcome, or n/a and why; `vs_base` names what base guaranteed and whether head keeps it, or `new`); extra dimensions are allowed; `cases` on other kinds is rejected.
  A `unit` entry must also carry `probe`: `{"cmd", "output"}` (both non-blank) or `{"why_not_run"}`; `probe` on other kinds is rejected.
  Verdict `fine` with `why_not_run` is rejected: an unrun probe makes the verdict `suspect`.
  A reviewer that suspects a defect it could not prove records `"verdict": "suspect"` with `"finding": null` and a
  `"suspicion": {"id": "<n>.s<k>", "claim", "evidence", "open_question", "tried"}` (all non-blank; ids unique per
  file and prefixed with the reviewer number); `suspicion` on any other verdict, or `suspect` without it, is rejected.
  `check`: schema; quote found within ±3 lines of the first site at head; every changed file has ≥1 ledger
  entry (except in a specialist's file, whose focus the helper cannot know, and the investigator's); every
  `finding` verdict names a finding in the file; every cited rule id is a file in a rules folder.
- `investigations.json` (written by the investigator subagent, which resolves suspicions): `{"items": [{"suspicion":
  "<n>.s<k>", "outcome": "confirmed" | "refuted" | "unresolved", "reason", "evidence", "finding": "<n>.<k>" | null,
  "check": text | null}]}`. Every suspicion in `reviews/*.json` appears exactly once and ids must exist. `confirmed`
  needs `finding`, a finding id present in some review file (the investigator appends it to its own
  `reviews/<m>.json`, lens `investigator`, m = next free number after the specialists, so `check` covers
  it); `unresolved`
  needs `check` (how a human verifies it quickly); `finding` is null unless confirmed, `check` null unless unresolved.
- `verified.json`: `{"findings": [{"id": "F<k>", …same fields…, "merged_from": ["<n>.<k>", …]}], "dropped":
  [{"from": "<n>.<k>", "refuted_by": site, "why"}]}`. `check`: every reviewer finding appears
  exactly once in `merged_from` or `dropped`; `refuted_by` exists at head. With `verified.json` present, `check` also
  requires `reviews/1.json..N.json` (N = 4 lens reviewers + the specialists `state.json` records as dispatched), unless `problem.json` has
  `blocks: true` and none of `reviews/1..N` exists (the gate stopped the review before reviewers ran), and, when any
  review holds a suspicion, `investigations.json`; confirmed findings are ordinary review findings and need the same
  exactly-once accounting.
- `report.md` (rendered by `sync` from `verified.json`, edited by the owner): one block per kept finding —
  `### F<k>. <title>` then `- post: yes | no`, `- severity: …`, `- where: site, site`, a blank line, the body
  (why it breaks, then `Fix: …`) up to the next `##`/`###` heading. `## My findings` takes `- site [severity] — text`.
  Unresolved suspicions render after the findings under `## Suspicions (unconfirmed)` as `### <claim>` with
  `- where:`, `- evidence:`, `- open question:` and `- check:` lines; skipped specialists then render under
  `## Specialists skipped` as `- <name>: <reason>`, so a wrong skip is visible. Reading back, only `post: yes` finding blocks and
  own findings matter: a bad post flag, severity or site, or a posted block without its fix, is an error with
  `report.md:line`; a site outside the diff is a warning (it would post as a general comment). Every other block and
  section (suspicions, notes, anything the owner adds) is ignored and never posted.
- `comments` prints, per `post: yes` finding and per own finding: `site` line, then
  `<label> (<blocking|non-blocking>): <title>` (conventional comments: issue / question / suggestion), body, fix;
  no ids, rule ids or tool mentions.

Resolved details (skill and helper agree on these):
- Review dir: every command takes an optional `<dir>` defaulting to the current directory and reads
  `<dir>/state.json`. The skill runs commands from the review dir.
- Core criterion = a criterion with a source quote (`implied: false`). `blocks` = any core criterion `not_met`, or no
  criterion `met`.
- Gate findings reach the verifier: `not_met` → blocking, `partly` → non-blocking, one question per scope-creep entry; their
  `merged_from` ids are `C<k>` and `S<k>`, which `check` accepts when they exist in problem.json and
  requires exactly once each in `merged_from` or `dropped`.
- Numbering: lens reviewers 1..4, then the dispatched specialists 5.. contiguous in load order (a skipped one
  takes no number) with `lens` = the specialist's name, then the investigator; `check` rejects any other review
  file numbered past the dispatched specialists. `rules` prints no specialist numbers, since it runs before the
  dispatch decisions.
- `revy-review rules` prints, per rules folder, a `Rule files: <dir>/<topic>/<id>.md` header, then
  `<id> (<topic>) [<langs>] — <When>`; then per specialist `Specialist: <name> (dispatch: <mode>) — <file>` with its own
  index indented two spaces below it.
- `report.md` may hold a `## Notes` section for the owner; `comments` never prints it.
- Exclusions everywhere use `post: no` (self mode: excluded from `fix`).
- Self mode = the owner's own change (no target, own commit/range, or `fix`); team mode = someone else's PR/MR.

## State
`~/.revy/reviews/<repo>/<target>/`: `state.json` (base, head, mode, round), `problem.json`, `reviews/<n>.json`,
`investigations.json`, `verified.json`, `report.md`.

## Rust helper (`revy-review …`)
The crate in `cli/`: one standalone `revy-review` binary; the skill in `skill/` calls it.
Pure and file-based except reading git objects for quotes and the diff.
- `rules [dir]`: validates the rules folders and specialists and prints the full rules index,
  with each specialist's dispatch mode and index.
- `check [dir]`: validates every file present in the review directory (Formats above) and the accounting across
  them — `problem.json`, each `reviews/<n>.json` (a file with lens `investigator` owes no ledger coverage, since it
  reviews only suspicions), `investigations.json` (every suspicion resolved exactly once, ids exist, `confirmed` names
  a finding in the investigator's file, `unresolved` carries a `check`), `state.json` `specialists` (the dispatch
  decisions), and `verified.json` (exactly-once accounting, every lens reviewer's and dispatched specialist's file, investigations for suspicions).
- `sync [dir]`: refuses a review that does not check clean; renders report.md from `verified.json` when absent,
  otherwise reads back the owner's edits.
- `comments [dir]`: post-ready comment text for findings with `post: yes` and own findings, from report.md alone
  (suspicions and notes are never printed).
Errors are precise and actionable; the skill tells the agent to fix and re-run until clean.

## Open (decided by eval)
- Final reviewer brief: `revy` candidate (ens2 + impact ledger + rules floor) vs tave on Opus 5.5 medium,
  on f01–f20 (shipped regressions) and h01–h08 (human-reviewed PRs).
- Rule-index filtering (`applies_to`).
