# checkout-persists-token
langs: github-actions

When: `actions/checkout` without `persist-credentials: false` in a job that later runs PR code, third-party actions or build tools, or uploads, caches or publishes the workspace.
Ask: `persist-credentials` defaults to `true`, so the job's token stays configured for git after checkout and every later step and process can read it: in `.git/config` inside the workspace before checkout v6, in a file under `$RUNNER_TEMP` from v6. Before v6, an upload of the workspace ships the token in `.git/config` when it includes hidden files: `include-hidden-files: true`, `upload-artifact` before v4.4 (v3 before 3.2), a tarball, an image build context. Does any later step push with git? If not, nothing needs the stored credential.
Not when: a later step in the same job pushes or tags with git and the job runs only trusted code.
Report as: `actions/checkout` in `<job>` leaves the token where `<later step / artifact>` can read it: `<untrusted code / artifact readers>` get `<token permissions>`; fix: `persist-credentials: false`, and give a token only to the step that pushes.
