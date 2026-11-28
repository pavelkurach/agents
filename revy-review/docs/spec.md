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
- `fix` (self mode): fully autonomous — the owner is away from invocation, so no step waits for them, before or
  inside the loop: capabilities proceed with what exists (install/enable suggestions go to the final summary); a
  gate without sources infers the goal from the commits (`source: inferred`); an existing review directory of a
  different base or head is renamed to `<dir>.<its head sha7>`; untracked files the snapshot omits and rules or
  specialist errors are skipped and listed in the final summary; a blocking gate's findings are fixed (round 0, not
  counted against the cap). Then review, fix, re-review, fix ….
  Each re-review targets only the fix range `<commit before the fixes>..<round commit>` (never the whole change
  again; impact still traced repo-wide); one commit per round (tests and fixes together), never push. Round 1's
  depth is chosen as for any review; round k+1's follows the severity of round k's fixes, not the `full`
  categories (those decide round 1 alone): `full` when a fix changes a signature, schema or wire/file/config format
  that consumers found in the repository outside the fix range read, else `focused` when any blocking finding was
  fixed, else `light` — never a loop to zero findings at full depth. The loop fixes what the change introduced or
  exposed: a finding identical at base (the original base; the same code path running there unchanged, so never
  a defect in code the change added) that no criterion covers is listed, not fixed. Fixes are chosen on their
  merits, never to steer the next round's depth; a round's `focus` names the findings fixed, never those left open. Questions are settled first, by subagents in
  parallel, one per question: a finding if confirmed, else answered in one line. Investigators are told the owner
  is away and run every check they can before answering `unresolved`; an unresolved suspicion goes to the final
  summary with its check, unfixed. A question subagent (strongest model, highest effort) follows
  `investigator.md` steps 1–2 with repo, head, `capabilities.md`, the path of `baseline.md`, the question's text and
  isolation n = `q<k>`; it writes no file and replies confirmed (input and wrong outcome) or a one-line answer.
  Re-reviews reuse `capabilities.md`, and `problem.json` for its statement and criteria only: gate items are
  accounted in round 1 alone. A round that fixed nothing (every defect not reproduced or excluded) ends the loop
  with no commit and no re-review.
  Any new blocking or non-blocking finding is fixed and another round runs, except after a `light` round, where
  only a new blocking finding does. The loop ends after a `light` round without one, or when the last round's
  verified findings hold no new blocking or non-blocking finding: that round's non-blocking findings and local,
  cheap questions and suggestions are applied (suite green, no further round), the rest listed. At most 5 rounds, a non-convergence
  guard: round 5's new findings are listed, not fixed, and the summary says what is still open. Every round still verifies and syncs
  `report.md`, which the fix step reads (findings, `post: no` exclusions), but the owner reads no fix-mode report:
  no per-round delivery, no explanation; the end is one short summary per round (depth, what it fixed, its
  commit), then what is still open: unresolved suspicions with their checks, defects not reproduced, listed minor
  findings, capability suggestions. Test first, for behavioural defects only: each one's proof test (cherry-picked
  from the proofs branch, or ported into the repo's test layout from a standalone script; a new failing test for a
  defect without proof where the code can run) lands on the branch and fails before the fix, and passes after it
  with the existing suite green; a defect whose test ran but could not be made to fail at head stays unfixed and is
  listed as not reproduced; one whose code cannot run here is fixed by reading and listed as fixed, not
  reproducible here; suggestions and design findings get no test. Each re-review's `baseline.md` is the fix step's
  last full suite run at the round commit.
  Exclusions: "fix all except 4" or `post: no` in report.md.

The agent resolves the target and records `base`/`head` in the review state; every later step uses them.

## Steps (SKILL.md)
0. Capabilities — the coordinator inventories, by purpose not product, code intelligence for each language the change
   touches, runtime execution (build and tests run here), structured and text search; reading source and grep are the
   floor. For each gap that would materially help, it names what the tool adds and its install/enable command, then
   waits for the owner (install, then re-inventory; or proceed without); with no such gap, or with `fix`, it
   proceeds without asking.
   It also works out the isolation recipe: from the repo's test config, everything a test run writes (databases,
   containers, compose project names, ports, temp/cache dirs, sockets, lock files) gets a per-subagent instance
   keyed by `<n>`; read-only caches stay shared; build output stays in each subagent's worktree, seeded by a
   copy-on-write clone of the checkout's build dir where the filesystem supports it. In team mode it settles
   revy-explain's path and renderer, asking the owner in the same batch when either is missing. Runs concurrently
   with 0b, which does not need it. Once the recipe is known, a background subagent runs the existing tests of the
   packages the change touches, once, at head, in its own worktree with n = `baseline`, and writes `baseline.md`
   (each command, its pass/fail summary, failing test names; the helper never reads it) — an input of every
   reviewer, specialist and investigator (by path), replacing each reviewer's own suite run; a reviewer reruns a
   suite only when its question needs a run the baseline did not cover, or when `baseline.md` is not written yet as
   it needs a result. Step 0 is done once the baseline subagent is dispatched.
   The final inventory, `capabilities.md` (`- <purpose>: <tool>` or `- <purpose>: none — <fallback>`, and
   `- isolation: <recipe, parameterized by <n>>` or `- isolation: none needed — <why>`; the helper never reads it),
   is an input of every reviewer, specialist, investigator and verifier, which use the strongest listed tool for
   each lookup and apply the recipe in their own worktree (the verifier with `<n>` = `verify`), tearing down what
   they started before replying.
0b. Problem gate — one agent (`problem.md`). Sources: ticket, PR/MR description, linked issue, commit messages; self mode
   without `fix` with none of them → ask the owner for a one-line goal; otherwise → infer from commits and mark it
   inferred.
   Writes `problem.json`: statement, acceptance criteria (each quoted from its source, or marked implied), out of scope,
   and per criterion `met | partly | not met | unverifiable` with `path:line` evidence; unrelated changes = scope creep.
   **Gate:** if the change does not address the problem, or a core criterion is `not met`, stop here: the review
   directory stays as the gate record, and any review that follows (after a gate fix, or team mode's deep review)
   is a fresh one in its own directory, `round` = 1, with `capabilities.md` copied: after a gate fix (at most one)
   of `base..<new head>` with 0b rerun there, stopping with the gate findings in the final summary when it still
   blocks; for team mode's deep review the same PR/MR target and team mode in `<dir>.deep`, `problem.json` copied
   since head is unchanged. Report only the
   gate findings (self mode: offer to fix first, or with `fix` fix them unasked and run the full review on
   `base..<new head>`; team mode: the report says so and asks whether to run the deep review
   anyway). `partly` and `unverifiable` do not block. Otherwise the criteria are passed to every reviewer as the
   scenarios that matter most, and the gate's findings join the reviewers' at the verifier.
1. Depth — the coordinator reads the diff stat and hunks and picks `full`, `focused` or `light` (unsure → the
   deeper one), recorded in `state.json` `depth`. `full`: the change alters a contract with consumers outside the
   diff, or touches concurrency, persistence/migrations, security/authz, money, cross-boundary error handling or
   public API. `focused`: logic changes whose consumers all sit in the diff or are few and local. `light`: no
   runtime behaviour reaches a consumer (tests, docs, formatting, renames fully inside the diff, a fix round after
   only minor fixes).
   Then L lens reviewers in parallel — `full` L = 4 (two lens pairs, each drawn twice), `focused` L = 2 (each pair
   once), `light` L = 1 (all four lenses, with a coordinator `focus` note on what the change is and where its risk
   lies; the note is optional at other depths and names the addressed findings in fix rounds) — same full brief, plus a context step
   (ticket and links, git log/blame of touched code, project docs) (`reviewer.md`: impact outward/inward,
   how-to-look, rules floor, impact ledger, structured findings) → `reviews/<n>.json`; each must pass `revy-review check`.
   Lenses (under test, revy3p): each reviewer also gets one "review everything, but go deepest on X" line —
   impact · failure and concurrency · tests and edge inputs · design and simplicity.
   Plus one subagent per specialist (see Extensions), same output schema and validation.
1b. Investigators (`investigator.md`, strongest model, highest effort) — the coordinator groups the suspicions by
   the code or question they share (one suspicion raised by several reviewers is one group) and dispatches one
   investigator per independent group, in parallel (few suspicions: one) → `investigations/<n>.json`, plus
   `reviews/<n>.json` (lens `investigator`) when it confirms anything. With `fix` they are told the owner is away
   and run every check they can here before answering `unresolved`.
2. One verifier (`verifier.md`): merge same-root-cause findings, walk each scenario in code, drop only when the code
   contradicts it (refuting only the stated reason is not enough — judge the fix on its merits), unresolved →
   question; a disputed finding (one reviewer's finding against another's `fine` at the same site or contract, or
   one walking cannot settle) — and, in team mode only, a blocking finding without a proof, as evidence for the
   author — is settled by running a test instead of becoming a question
   → `verified.json`; must pass `revy-review check`. Gate items merge only in round 1. With no reviewer finding
   and, in round 1, no gate item (no `not_met` or `partly` criterion, no scope creep) the verifier is skipped: the coordinator writes
   `{"findings": [], "dropped": []}` and deletes the per-reviewer proofs branches and worktrees.
3. `revy-review sync` renders `report.md` (editable) from `verified.json`. Team mode: a background subagent then
   reads revy-explain's SKILL.md by path (it is not model-invocable) and runs its `review-report` mode on report.md
   at head, its step-1 questions pre-answered (scope = the report, renderer from `~/.revy-explain/config.toml`);
   the owner gets the summary at once and the explanation's path when it lands, while curation goes on.
4. Self mode: deliver report, or run the `fix` loop. Team mode: curate → post.

## Proofs
- A probe is a new test that drives a unit's riskiest edge-case cells through the changed code — in the repo's own
  test framework and layout where it can run, a standalone script otherwise; rerunning the existing suite or a search
  is not a probe. It runs only where running decides what reading cannot (concurrency and interleavings, cost at
  scale, dependency/runtime/database/OS behaviour, numeric precision and overflow, parsing and encoding, any doubted
  cell, any vs_base claim about cost or ordering); otherwise the unit records `settled_by_reading`. The edge-case
  matrix stays mandatory and exhaustive. `why_not_run` stays for code that cannot run here.
- A reviewer or investigator commits a proof test only for a finding that is blocking or whose claim rests on
  runtime behaviour the code alone does not show; any other finding stands on its quote and `breaks`. The verifier
  runs a test for each disputed finding, and in team mode for each blocking one without a proof (self mode's fix
  loop reproduces those test-first), and commits a proving one onto `revy/<h>/proofs`.
- Tests and experiments never touch the reviewed branch. Each reviewer and investigator n works in its own git
  worktree `<review dir>/scratch/proofs-<n>` on local branch `revy/<h>/proofs-<n>` from head (`<h>` = first 7
  characters of head), created before its first run, and commits each proof test there. The verifier cherry-picks one proving commit per kept
  finding onto `revy/<h>/proofs` from head, reruns each, and cites the new shas; then it deletes every
  `revy/<h>/proofs-<n>` branch and its worktree, and `revy/<h>/proofs` too when no kept finding is proven. Branches live in the repo's refs, so
  they survive the deletion of `scratch/` (followed by `git worktree prune`); none is ever pushed. Team mode never
  touches the PR branch; self mode's `fix` loop moves the proof tests onto the branch as regression tests.

## Extensions
- Rules: see Rules.
- Specialists: `reviewers/<name>.md` in the skill (the built-in focused reviewers) and in `~/.revy-review/` (the
  owner's), loaded in that order, file-name order within each; the file stem is the name, unique across both (a
  duplicate is an error naming both files). A file is optional header lines in any order — `rules: <topic>, …`
  and `dispatch: always | when relevant` (absent: `always`; any other value is an error naming the file) — then
  the prompt (the focus). An `always` specialist runs on every review; a `when relevant` one runs when the
  coordinator judges, from its prompt and the change, that the change has something for it, and when unsure it
  runs — no path or extension matching. The coordinator records every decision in `state.json` `specialists`.
  The dispatched specialists run as reviewer n = L+1, L+2, … in load order after the L lens reviewers, with
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
  (`issue (blocking): …`), why it breaks, the reproducing test's code and run line when proven, fix. No process artifacts (ids, rule
  ids, review-directory paths, tool mentions). Optional
  footer is config, off by default. Tone follows the owner's `voice.md`.
- The agent posts them as a **draft/pending review** with whatever forge tool exists, after confirming the scope;
  verifies each landed inline where intended.

## Formats (contract between the skill and the helper)
All JSON, UTF-8. `site` = `"path:line"` (repo-relative, line at `head`). Ids are stable strings.
- `state.json`: `{"repo": abs path, "target": text, "base": sha, "head": sha, "mode": "self" | "team",
  "round": n, "skill": abs path, "specialists": {name: "dispatched" | {"skipped": reason}},
  "depth": {"level": "light" | "focused" | "full", "why": one line}}` — written by the
  agent; every command reads it from the review directory. `mode` steers only the skill; `round` is required
  (≥ 1) and decides gate-item accounting and how `problem.json` is checked.
  `depth` is written before the reviewers are dispatched: `check` requires it, with a non-blank `why`, once any
  `reviews/*.json` exists, and it may be absent before that. Its level sets L, the number of lens reviewers:
  `full` 4, `focused` 2, `light` 1.
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
  `S<k>`, unique. In fix rounds (`round` > 1) `problem.json` is a copy whose sites point at an older head: `check`
  validates its schema only — no site resolution at head, no `blocks` recomputation.
- `reviews/<n>.json`: `{"reviewer": n, "lens": text | null, "ledger": [{"kind": "contract" | "unit", "subject":
  contract text or unit name, "site", "verdict": "fine" | "finding", "finding": finding id | null, "cases": {dimension: text, …} (unit only)}],
  "findings": [{"id": "<n>.<k>", "title", "severity": "blocking" | "non-blocking" | "question" | "suggestion",
  "sites": [site, …], "quote": exact line at the first site, "breaks", "fix", "rule": rule id | null,
  "proof": {"test": "<path>::<test name or line>", "commit": sha, "cmd", "head": failing output excerpt, "base":
  output at base | "n/a"} (optional)}]}`. A `proof` is checked strictly: every field non-blank, `test` splits at its
  first `::` into a repo-relative path and a name, `commit` is a commit in the repository and `test`'s path exists in
  it. A finding without `proof` keeps the evidence rules above.
  A `unit` entry must carry `cases` answering every floor
  dimension (size, boundaries, absence, shape, variants, state, failure, concurrency, vs_base) with a non-empty text (the case
  walked and its outcome, or n/a and why; `vs_base` names what base guaranteed and whether head keeps it, or `new`); extra dimensions are allowed; `cases` on other kinds is rejected.
  A `unit` entry must also carry `probe`: `{"cmd", "output"}` (both non-blank), `{"why_not_run"}` or
  `{"settled_by_reading": why reading settles every risky cell}` (non-blank); `probe` on other kinds is rejected.
  Verdict `fine` with `why_not_run` is rejected: an unrun probe makes the verdict `suspect`. Verdict `suspect` with
  `settled_by_reading` is rejected: a doubt reading did not settle needs a run or `why_not_run`.
  A reviewer that suspects a defect it could not prove records `"verdict": "suspect"` with `"finding": null` and a
  `"suspicion": {"id": "<n>.s<k>", "claim", "evidence", "open_question", "tried"}` (all non-blank; ids unique per
  file and prefixed with the reviewer number); `suspicion` on any other verdict, or `suspect` without it, is rejected.
  `check`: schema; quote found within ±3 lines of the first site at head; every changed file has ≥1 ledger
  entry (except in a specialist's file, whose focus the helper cannot know, and the investigator's); every
  `finding` verdict names a finding in the file; every cited rule id is a file in a rules folder.
- `investigations/<n>.json` (one per investigator subagent, n a distinct number after the dispatched specialists,
  not necessarily contiguous; each resolves its group of suspicions): `{"items": [{"suspicion":
  "<n>.s<k>", "outcome": "confirmed" | "refuted" | "unresolved", "reason", "evidence", "finding": "<n>.<k>" | null,
  "check": text | null}]}`. The union of `investigations/*.json` resolves every suspicion in the reviews exactly once (an id in two files is an error naming both; an
  investigator's own suspicion must sit in its own shard) and every id must exist. `confirmed`
  needs `finding`, a finding in `reviews/<n>.json` of the same n, lens `investigator` (the investigator writes it
  there, so `check` covers it); `unresolved`
  needs `check` (how a human verifies it quickly); `finding` is null unless confirmed, `check` null unless unresolved.
  An `investigations/<n>.json` whose n is not after the dispatched specialists is an error; a review file with lens
  `investigator` needs its `investigations/<n>.json` and must number after the dispatched specialists. A `suspect`
  entry in it is a new doubt the investigator raised: `check` counts it among the suspicions and requires its item
  in the same n's shard (usually `unresolved`), and `sync` renders it under Suspicions when unresolved. A legacy `investigations.json` is
  an error: "old single-file format; move it to investigations/<n>.json, n = the investigator's reviewer number".
- `verified.json`: `{"findings": [{"id": "F<k>", …same fields…, "merged_from": ["<n>.<k>", …]}], "dropped":
  [{"from": "<n>.<k>", "refuted_by": site, "why"}]}` (`proof` optional, as in reviews). `check`: every reviewer finding appears
  exactly once in `merged_from` or `dropped`; `refuted_by` exists at head; a kept finding's `proof.commit` is also
  reachable from `refs/heads/revy/<h>/proofs`, the branch report.md names. Once `verified.json` exists, only its
  proofs are checked against git: the per-reviewer branches are deleted, so their commits may be pruned. With `verified.json` present, `check` also
  requires `reviews/1.json..N.json` (N = L lens reviewers + the specialists `state.json` records as dispatched), unless `problem.json` has
  `blocks: true` and none of `reviews/1..N` exists (the gate stopped the review before reviewers ran), and, when any
  review holds a suspicion, investigations covering every suspicion across the shards; confirmed findings are ordinary review findings and need the same
  exactly-once accounting.
- `report.md` (rendered by `sync` from `verified.json`, edited by the owner): one block per kept finding —
  `### F<k>. <title>` then `- post: yes | no`, `- severity: …`, `- where: site, site`, a blank line, the body
  (why it breaks, then `Fix: …`) up to the next `##`/`###` heading. A proven finding's body holds, between why and
  fix, ``Reproduced by `<test>` — `<cmd>`:``, the test's code in a fenced block (info string: the test file's
  extension), `At head:` and its failing excerpt indented as code, then `At base:` and the base output unless `base`
  is `n/a`. `sync` reads the code from the proof commit (`git show <commit>:<path>`): the test named by the last
  `::` segment of `test` (parameters and subtests cut), with the attributes or decorators directly above it,
  through its closing brace or its indented body; when no such test is found (a line number, an unmatched name),
  the test file's diff in that commit, fenced as `diff`. Read-back skips headings inside a code fence; after the findings, `## Proofs` names the branch and how to run or drop it. `## My findings` takes `- site [severity] — text`.
  Unresolved suspicions render after the findings under `## Suspicions (unconfirmed)` as `### <claim>` with
  `- where:`, `- evidence:`, `- open question:` and `- check:` lines; skipped specialists then render under
  `## Specialists skipped` as `- <name>: <reason>`, so a wrong skip is visible, and `state.json` `depth` renders
  under `## Review depth` as `- <level>: <why>`, so a wrong depth call is visible. Reading back, only `post: yes` finding blocks and
  own findings matter: a bad post flag, severity or site, or a posted block without its fix, is an error with
  `report.md:line`; a site outside the diff is a warning (it would post as a general comment). Every other block and
  section (suspicions, notes, anything the owner adds) is ignored and never posted.
- `comments` prints, per `post: yes` finding and per own finding: `site` line, then
  `<label> (<blocking|non-blocking>): <title>` (conventional comments: issue / question / suggestion), body (with
  its reproduction), fix; no ids, rule ids or tool mentions; `## Proofs` is never printed.

Resolved details (skill and helper agree on these):
- Review dir: every command takes an optional `<dir>` defaulting to the current directory and reads
  `<dir>/state.json`. The skill runs commands from the review dir.
- Core criterion = a criterion with a source quote (`implied: false`). `blocks` = any core criterion `not_met`, or no
  criterion `met`.
- Gate findings reach the verifier: `not_met` → blocking, `partly` → non-blocking, one question per scope-creep entry; their
  `merged_from` ids are `C<k>` and `S<k>`, which `check` accepts when they exist in problem.json and
  requires exactly once each in `merged_from` or `dropped` — only when `state.json` `round` = 1; later rounds
  expect no gate items.
- Numbering: lens reviewers 1..L (L by `depth`), then the dispatched specialists L+1.. contiguous in load order (a
  skipped one takes no number) with `lens` = the specialist's name, then the investigators, each a distinct number
  after them, not necessarily contiguous; `check`
  rejects any other review file numbered past the dispatched specialists. `rules` prints no specialist numbers, since it runs before the
  dispatch decisions.
- `revy-review rules` prints, per rules folder, a `Rule files: <dir>/<topic>/<id>.md` header, then
  `<id> (<topic>) [<langs>] — <When>`; then per specialist `Specialist: <name> (dispatch: <mode>) — <file>` with its own
  index indented two spaces below it.
- `report.md` may hold a `## Notes` section for the owner; `comments` never prints it.
- Exclusions everywhere use `post: no` (self mode: excluded from `fix`).
- Self mode = the owner's own change (no target, own commit/range, or `fix`); team mode = someone else's PR/MR.

## State
`~/.revy/reviews/<repo>/<target>/`: `state.json` (base, head, mode, round, specialists, depth), `capabilities.md`,
`baseline.md`,
`problem.json`, `reviews/<n>.json`, `investigations/<n>.json`, `verified.json`, `report.md`.

## Rust helper (`revy-review …`)
The crate in `cli/`: one standalone `revy-review` binary; the skill in `skill/` calls it.
Pure and file-based except reading git objects for quotes and the diff.
- `rules [dir]`: validates the rules folders and specialists and prints the full rules index,
  with each specialist's dispatch mode and index.
- `check [dir]`: validates every file present in the review directory (Formats above) and the accounting across
  them — `problem.json` (schema only when `round` > 1), each `reviews/<n>.json` (a file with lens `investigator` owes no ledger coverage, since it
  reviews only suspicions), `investigations/<n>.json` (their union resolves every suspicion exactly once, ids exist,
  `confirmed` names a finding in the same n's investigator file, `unresolved` carries a `check`; a shard numbered
  at or before the specialists, an investigator review without its shard, an investigator's own suspicion resolved
  outside its shard, and a legacy
  `investigations.json` are errors), `state.json` `specialists` (the dispatch
  decisions) and `depth`, and `verified.json` (exactly-once accounting, gate items in round 1 only, every lens reviewer's and dispatched
  specialist's file, investigations for suspicions).
- `sync [dir]`: refuses a review that does not check clean; renders report.md from `verified.json` when absent,
  otherwise reads back the owner's edits.
- `comments [dir]`: post-ready comment text for findings with `post: yes` and own findings, from report.md alone
  (suspicions and notes are never printed).
Errors are precise and actionable; the skill tells the agent to fix and re-run until clean.

## Open (decided by eval)
- Final reviewer brief: `revy` candidate (ens2 + impact ledger + rules floor) vs tave on Opus 5.5 medium,
  on f01–f20 (shipped regressions) and h01–h08 (human-reviewed PRs).
- Rule-index filtering (`applies_to`).
