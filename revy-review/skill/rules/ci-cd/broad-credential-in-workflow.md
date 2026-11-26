# broad-credential-in-workflow
langs: github-actions, gitlab-ci

When: Workflow or pipeline uses a personal access token, SSH key or long-lived cloud key from a secret or CI variable for git, the GitHub or GitLab API, or a cloud provider.
Ask: Anyone who can push a branch can usually read every repository secret: a push-triggered workflow runs from the pushed file, so its branch filter and steps are the pusher's. A classic PAT or a personal SSH key reaches every repository its owner can, so storing one hands that reach to every writer of this repository. Is more than the job's own token (one repository, expires with the job) needed? If so, is it the narrowest kind: a deploy key (one repository, git only), a GitHub App installation token on selected repositories, a fine-grained or project token of a dedicated account, OIDC (`id-token: write`, `id_tokens:`) for cloud access?
Not when: an environment or protected-variable secret that only protected branches can read, and the job runs no untrusted code.
Report as: `<step>` uses `<credential>`, which reaches `<scope>`: every user who can push to `<repository>` gets that access; fix: the job's own token, a deploy key or App installation token scoped to `<repositories>`, or OIDC for `<cloud>`.
