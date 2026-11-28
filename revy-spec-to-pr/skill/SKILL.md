---
name: revy-spec-to-pr
description: Turn one PR-sized spec — a revy-plan slice or a spec file — into an implemented, self-reviewed draft pull request. Invoked as /revy-spec-to-pr <plan.md> <slice name> | <spec file>.
disable-model-invocation: true
argument-hint: "<plan.md> <slice name> | <spec file>"
---

# revy-spec-to-pr

You are the **dispatcher**: your job is to dispatch three fresh subagents in turn — implementer, reviewer, PR
opener — relay messages between them and your invoker, and keep `state.md`. Every other action, however small,
is a subagent's. Each subagent gets only the paths it needs and returns a short result, so no context overloads.

A subagent runs a skill through the Skill tool; when the tool does not list it, it reads
`~/.claude/skills/<name>/SKILL.md` and follows it.

**Relay**: a subagent cannot reach the user. A question it returns goes to your invoker — the user, or your caller
when you run as a subagent, by ending your turn with it — and the answer goes back to that same subagent through
SendMessage, so its context is kept.

## State

Slug, ≤ 40 chars in kebab case: `<plan slug>-<slice name>` for a plan slice, the file's stem for a spec file.
Directory: `~/.revy-spec-to-pr/<slug>/`. Its `state.md` holds `Step` (the current step and what remains; update it
after every step), the input, the branch, the worktree, each subagent's id and its returned result. An existing
`state.md` is this run: resume from `Step`.

## 1. Implement

Dispatch a fresh implementer, strong model, with the input (plan path and slice name, or spec file path), the
slug, the directory, and the branch and worktree `state.md` records, if any. Its brief:
- the spec is the slice's block in the plan's Delivery section, with the signatures, criteria and constraints of
  the plan it names; a name matching no slice is an error listing the plan's slices; or the spec is the file;
- with a recorded branch and worktree, work in them; otherwise `git fetch origin`, then
  `git worktree add -b <slug> <directory>/<repo dir name> origin/<default branch>` — the worktree keeps the
  repository's name because revy-review names its review directory after it; an unrecorded branch or worktree of
  that name already there is an error to return;
- build exactly the spec: every design decision is the spec's; a gap, or a spec claim the code contradicts,
  comes back as a question;
- reach **green** per AGENTS.md § Implementation Completion — the spec's verify and tests, plus the suites of
  the packages it touched;
- commit once on the branch: subject = the spec's goal, body = its acceptance criteria verbatim (revy-review's
  problem gate reads them as its sources); stage only the files it changed; never push;
- return the worktree path, the default branch, the commit sha, each test command with its result, and open
  questions.

**Done:** the implementer returned a commit with every reported run green, and every question is answered.

## 2. Review

Dispatch a fresh reviewer, strong model, given only the worktree path, the directory and the target. From the
worktree it runs revy-review with that target and `fix`; the target is none — the branch against its merge-base
with the default branch — except in § Changes; revy-review owns the loop. It writes revy-review's final summary
verbatim to `<directory>/review-summary.md` and returns that path, every round's `report.md` path, the render path
of the unconfirmed suspicions' explanation or `none`, and the blocking findings still open when the loop stopped
at its round cap, or `none`.

**Done:** the reviewer returned all four.

## 3. Open the PR

Dispatch a fresh PR opener, fast model, with the worktree path, the spec input, `review-summary.md`'s path and the
open blocking findings. It runs `git push -u origin <slug>` and `gh pr create --draft --base <default branch>
--head <slug>`, titled with the spec's goal; the body is the open blocking findings first, when any, then the
spec, then the summary verbatim. It returns the URL that `gh pr view` shows as an open draft.

**Done:** the PR opener returned the draft PR's URL.

## Changes

When the invoker asks for changes to the open PR, with their comments:
1. Dispatch a fresh implementer in the recorded worktree and branch, given the comments and the spec input; it
   applies them, reaches **green**, commits, and returns as in step 1.
2. Step 2, with target `<branch head before the change>..HEAD`.
3. A fresh PR opener runs `git push` to the same branch, so the same PR updates, and returns its URL.

**Done:** the PR shows the new commits, and the invoker has the Return.

## Return

To your invoker: the draft PR's URL; the open blocking findings, stated explicitly, or `none`; the
suspicions' render path, or `none`; every review `report.md` path.
