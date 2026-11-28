---
name: revy-plan
description: Use for `/revy-plan`, technical specs, design requests, or implementation-plan requests for non-trivial code changes before implementation.
---

# revy-plan

Produce a plan an implementer can execute without making a single design decision, and that introduces no bug
or inconsistency: the full impact of the change traced, every requirement clarified, every claim about existing
code backed by evidence. That bar is fixed, whatever the diff size. Your judgement sets the method and the
effort — which tools, which reviewers, whether a patch needs re-review — so effort goes where risk lives.

The reader arrives as a newcomer to the system and must decide as its **owner**, with the full picture. Assume
nothing they bring, and dive to the bottom of every part the change touches: the plan, and every question put to
them, carries why the system is the way it is, what it guarantees, where it breaks, and what each choice trades
away — never a default to accept on trust.

You coordinate; subagents do the tool-heavy work. Brief each one with only what its task needs — the goal, the
paths of the files to read, the repo root, the capabilities, an output path — and require a short return: the
result, how it was verified, risks, open questions. Tool output stays inside the subagent. Run independent
subagents together.

## Files

Plan directory: `~/.revy-plan/plans/<slug>/`.

| File | Holds | Reader |
|---|---|---|
| `plan.md` | the plan as it stands — only what is needed to act on it, diagrams included | the user, the implementer, reviewers |
| `log.md` | how it was built, under these headings: `Status`, `Problem`, `Capabilities`, `Evidence`, `Decisions`, `Reviews`, `Patches` | reviewers (Problem, Capabilities, Evidence), you on resume |
| `presentation.md` | how `plan.md` should look — reading order, visual form, emphasis | the renderer |
| `scratch/` | subagent outputs, checkpoints, worktrees | subagents |

`Status` names the current step and what remains; update it after every step. When the plan directory already
exists, confirm its `Problem` matches the request, then resume from `Status`.

## Baseline

The user may have run `revy-explain` on this task first. Before step 2, read the `Request` of each
`~/.revy-explain/explanations/*/log.md`; when one matches the task, confirm it with the user and record its path
in `Problem`. Its `explanation.md`, FAQ included, is the user's **baseline**: what they have already been shown.
Without one, the baseline is empty.

- Its claims and its `log.md` § `Evidence` are **leads**, never evidence: they point explorers where to look,
  and each claim the plan relies on is proven afresh per [evidence.md](references/evidence.md).
- When your evidence contradicts a baseline claim, tell the user in the next question batch — their
  understanding rests on it.
- Every question you put to the user, at any step, carries the context they lack: name the baseline section that
  already covers it, and give inline only what the baseline leaves out — terse, plain English per
  [writing.md](references/writing.md), a diagram when it has steps, naming the code it rests on.

## Steps

### 1. Capabilities

Inventory by purpose: code intelligence (definitions, references, call hierarchy) per language touched; the
project's own build and test commands; structured and text search. A tool the harness loads on demand counts as available.
Reading source and grep are the floor. When a missing capability would materially sharpen evidence — a language server, a code-graph tool — propose it with
its install command and wait for the user's answer.

**Done:** `Capabilities` lists each capability or its fallback, and every proposal has the user's answer.

### 2. Frame and clarify

Write `Problem`: the request as given, then the goal, constraints, and acceptance intent. Ask the user every
question whose answer changes the design, batched, each with your recommended default, and log each question
with its answer. The settled scope goes into `plan.md`. Ask again whenever a later step surfaces
a new one.

**Done:** every requirement is stated by the user or is an assumption the user confirmed.

### 3. Explore

Gather evidence per [evidence.md](references/evidence.md). Dispatch one explorer subagent per independent area
(a service, a repository, a call path), or one per open question; read a small area yourself. For a bug, the reproduction comes first, then a deep root cause analysis.

**Done:** entry points, consumers, tests, and config of the affected behavior are enumerated in full; the
owner's depth of the touched area is recorded; every
impact lane is accounted for; every claim about existing code has an item in `Evidence`.

### 4. Challenge and decide

Push back once the evidence is in: does the change solve the stated problem, is there a smaller one, does it
break a consumer or invariant the requester did not know about? Each challenge cites evidence. Then lay out the
decisions per [design.md](references/design.md); every one-way decision and every fork the user owns goes to
the user with your recommendation.

**Done:** `Decisions` holds every decision with its owner; each challenge and user-owned decision has the
user's answer; with no challenge, `Decisions` says what you checked.

### 5. Draft

Write `plan.md` per [writing.md](references/writing.md) and [design.md](references/design.md). Drafting is
judgement over evidence — do it in one subagent given the paths of `log.md` and the references, or yourself
when the context is already in hand.

**Done:** the checks in design.md § Slices pass; every planned test earns its place per design.md § Tests and
carries its status; every seam is proven per design.md § Test seams, with its runs
in `Evidence`.

### 6. Review the draft

Dispatch at least one lens from [reviewers.md](references/reviewers.md), chosen by what the plan touches, each
as its own fresh subagent, all together.

**Done:** every lens has returned, its findings file sits in `scratch/` and is linked from `Reviews`, and every
skipped lens has a one-line reason there.

### 7. Converge

Settle every finding per reviewers.md § Converging, logging one line per finding in `Reviews` — its id and
disposition — and applying each fix as a patch (below).

**Done:** no unresolved must-fix finding remains, and every escalation has the user's ruling.

### 8. Present

Write `presentation.md` per [writing.md](references/writing.md). Read `renderer` from `~/.revy-plan/config.toml`;
when the file or key is absent, ask the user which installed skill renders plans and save the answer as
`renderer = "<skill>"`. Dispatch a subagent to invoke that skill with the paths of `plan.md` and
`presentation.md`, `log.md` as the evidence ledger, the repository root(s), and an output directory inside the
plan directory. Tell it `plan.md` is the content, verbatim:
it renders every item and every diagram edge as written, and `presentation.md` decides only the look. When the
skill is missing or fails, report the error and ask the user for another.

**Done:** every `plan.md` item and diagram edge appears in the render, and the user has its path plus the plan's blind spot —
the one decision no test can check, or what checks every decision when there is none.

## Patching

Every change after the first review — a review fix, user feedback, a later revision — is a patch to `plan.md`.
Log each one in `Patches`: what changed, why, which sections it touches, the re-review it got.

Re-review by impact, scoped to what moved. A patch spanning rows takes the highest.

| Patch | Re-review |
|---|---|
| Local and additive — a new test scenario or criterion (with the test file it lives in), wording, presentation | none, or a quick check when a fresh eye would catch something |
| Removes or merges a criterion or test | a quick check that everything it covered is still covered |
| Changes a contract, flow, decision, data shape, rollout step, slice boundary, or the evidence a claim rests on | the lenses whose subject changed, on the changed sections and the sections that depend on them |
| Changes the approach or scope — every section rests on it | every relevant lens on the whole plan |

When a patch or finding needs facts the evidence lacks, dispatch an explorer for exactly that question before
patching.

A patch that changes a user-owned decision goes to the user before it lands.
