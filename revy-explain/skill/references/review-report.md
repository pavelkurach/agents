# review-report

The context a reader needs to understand every finding in a code review report. The report is the source of
truth: explain each finding as written — add none, drop none, re-rate none. When evidence contradicts a finding's
premise (the cited code is not there, the path cannot be reached), show the contradiction with its evidence ids
under that finding; the finding stays.

Copy the report verbatim to `<dir>/report.md` — from a path, pasted text, or the PR's review comments — and cite
it as `report · <finding id>` (`via:report`). Check out the revision it reviewed — its commit, branch, or PR
head — per evidence.md § Worktrees. When the report names none, use the working tree and say so under Gaps.

## Enumerate

| Enumerate | Scope |
|---|---|
| Findings | every finding in the report, in report order — its id, severity, site, and claim, verbatim |
| Sites | the code each finding cites, at the reviewed revision |
| Flows | the flow each finding runs through, trigger to failure — its entry points, actors, stages, and branches per how-it-works.md § Enumerate |
| Triggers | the input or sequence that makes each finding bite, as the report states it; reproduced when runnable |
| Causes | per finding, a deep root cause analysis per evidence.md § Root cause |
| Effects | what each finding does to the user, the data, and the consumers |
| Shared ground | findings that share a site, a flow, or a cause |

## Body

1. **At a glance** — a table: finding, severity, site, what breaks in one clause. The Map marks where each
   finding sits.
2. **Per finding**, in report order, each under a heading `### <finding id> — <title>`, so every mention of the
   id links to it:
   - the finding, quoted;
   - the flow as a diagram, the failing edge marked `failure` and the site `focus`;
   - the code at the site, as a code block;
   - the trigger as given / when / then rows, with the reproduction when one ran;
   - the cause as a flowchart of the chain, each hop cited, the root cause marked `failure`, then what else it
     breaks;
   - the effect — what the user sees, what data or consumer is hurt;
   - the report's suggested fix, quoted, when it has one.
3. **Shared ground** — a diagram linking findings that share a site, a flow, or a cause.
