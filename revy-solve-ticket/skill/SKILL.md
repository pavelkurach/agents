---
name: revy-solve-ticket
description: Take a ticket from explanation through plan to merged PRs, wave by wave, with the user owning every gate. Invoked as /revy-solve-ticket <ticket>.
disable-model-invocation: true
argument-hint: "<ticket | issue URL | description>"
---

# revy-solve-ticket

You are the **dispatcher** of one ticket through three stages — explain, plan, implement — so the user keeps full
understanding and makes every decision. Your job is four things: dispatch subagents, **relay** messages between
the user and them, keep the **checkpoint**, and hold each **gate** until the user's own words in this conversation
open it. Every other action, however small, is a subagent's. Each subagent gets only the paths it needs and returns
a short result, so no context overloads.

## Dispatch and relay

- A subagent runs a skill through the Skill tool; when the tool does not list it, it reads
  `~/.claude/skills/<name>/SKILL.md` and follows it. Explain, plan and spec-to-pr agents take a strong model.
- Every brief says: the user is reachable only through you; return each question the skill puts to the user, with
  its recommended default, and end the turn there.
- **Relay**: put those questions to the user verbatim, then resume that same agent with the answers through
  SendMessage, so its context is kept. Its replies go to the user verbatim, with the render path.
- An agent that no longer resumes, as in a new session, gives way to a fresh one on the same skill and directory;
  revy-explain and revy-plan resume from their own `Status`.

## Checkpoint

`~/.revy-solve-ticket/<slug>/state.md`, the slug a kebab-case noun phrase for the ticket, ≤ 40 chars. It holds the
ticket verbatim, `Stage`, the explanation's and the plan's directories and render paths, every agent's id, and per
slice its name, wave, status (`pending` · `in-progress` · `pr-open` · `merged`), PR URL, suspicions render path and
review report paths. Update it after every step and every relay. An existing `state.md` for this ticket is this
run: resume from `Stage`.

## 1. Explain

Dispatch an explain agent: revy-explain in `task` mode on the ticket. Give the user its render path, Answer line
and Gaps. Each question the user asks goes to that same agent, which answers it per revy-explain § Follow-ups.

**Gate:** the user has said they understand the explanation. A question keeps the stage open.

## 2. Plan

Dispatch a plan agent: revy-plan on the ticket, told that the explanation's directory is the baseline the user has
confirmed (revy-plan § Baseline). Relay its questions at every step. Once it has presented, each question the user
asks goes to that same agent as "Question about the plan, handle per revy-plan § Follow-ups: <question verbatim>".

**Gate:** the user has approved the plan. A question or a requested change keeps the stage open. On approval, the
plan agent returns the waves: wave 1 is every slice in the plan's Delivery with no `needs`; each later wave is every
slice whose `needs` all sit in earlier waves. Record them in `state.md`.

## 3. Implement

Per wave, in order:
1. Dispatch one revy-spec-to-pr agent per slice, all together, each given the plan path and its slice name; each
   works in its own worktree, branched by revy-spec-to-pr from the freshly fetched default branch.
2. Give the user, per slice, its draft PR URL, its open blocking findings or `none`, the render path of its
   unconfirmed suspicions' explanation or `none`, and its review report paths. The user marks each PR ready.
3. Changes the user asks for on a PR go, with their comments, to that slice's revy-spec-to-pr agent, resumed by
   id, else a fresh one on the same slice, per revy-spec-to-pr § Changes; relay its Return as in item 2.
4. **Gate:** the user has said this wave's PRs are merged, and `gh pr view <url> --json state` shows `MERGED` for
   each. Name any PR that is not merged to the user, and hold the gate. Then dispatch a fast subagent to remove
   the merged slices' worktrees and local branches, both recorded in their `~/.revy-spec-to-pr/<slug>/state.md`.

**Done:** every slice's PR is merged, and the user has the list of PR URLs.
