# privileged-run-trusts-pr-artifact
langs: github-actions

When: `workflow_run` job downloads or reads the triggering run's artifacts (`actions/download-artifact` with `run-id`, `gh run download`, the API).
Ask: When the triggering workflow runs on `pull_request`, every byte of the artifact was chosen by the PR author. Is it extracted outside the workspace (`${{ runner.temp }}`), where it cannot replace a script, config or local action that a later step runs? Is each value parsed to its narrowest type before use, and is a parsed PR number used only where any PR's number is harmless, such as posting a comment? Does anything from it run: a binary, a script, a report rendered or interpreted downstream?
Report as: `<job>` on `workflow_run` `<extracts into the workspace / uses unparsed / executes>` `<artifact>` from an untrusted PR run: the PR author controls what the privileged job runs or writes; fix: download to `runner.temp`, parse values to types (find the PR with `pulls.list` by `workflow_run.head_sha` instead of trusting a number from the artifact), and run nothing from it.
