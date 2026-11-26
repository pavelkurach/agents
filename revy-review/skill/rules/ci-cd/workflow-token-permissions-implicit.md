# workflow-token-permissions-implicit
langs: github-actions

When: Workflow or job added or changed with no `permissions:` key, with `write-all`, or with write scopes that only some of its steps need.
Ask: Without `permissions:`, `GITHUB_TOKEN` gets the repository's default, which the diff does not show: read-only (`contents` and `packages` read) for enterprises, organizations outside an enterprise and personal repositories created since 2 February 2023, read and write on every scope for those created earlier; an organization inherits its enterprise's setting and a repository its organization's. Once any scope is listed, every unlisted scope is `none`. Which steps need write, and can they move to their own job? Expiry at the end of the job does not limit the damage: injected code sends the token out and uses it while the job runs.
Not when: `permissions: {}` or read-only scopes at workflow level, with writes granted on the one job that uses them.
Report as: `<job>` runs with `<default / write-all / broad>` token permissions: any injected or compromised step can push to `<branches>`, retag releases or edit issues; fix: `permissions: contents: read` at workflow level and each write scope only on the job that needs it.
