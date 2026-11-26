# terraform-plan-on-untrusted-change
langs: terraform, github-actions, gitlab-ci

When: CI job runs `terraform plan` or `tofu plan` on a PR or MR branch with cloud or backend credentials.
Ask: Plan is not read-only: it runs the providers the change names, and reads data sources during plan, so `data "external"` runs any program and a custom provider runs any code, with the job's credentials. Who can open the change that the job plans, and does the plan job hold more than read-only credentials and a state backend it can lock?
Not when: the job runs only `terraform fmt` or `validate`, or `plan` with no credentials and a local backend.
Report as: `<job>` runs `terraform plan` on `<untrusted ref>` with `<credentials>`: a change adding `data "external"` or a provider runs code with those credentials; fix: plan untrusted changes with read-only credentials in an isolated job, and keep write credentials for `apply` on reviewed, merged code.
