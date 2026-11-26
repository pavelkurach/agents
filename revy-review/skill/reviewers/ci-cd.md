rules: ci-cd, security
dispatch: when relevant

Review the CI/CD definitions this change adds or alters, and what they run: GitHub Actions workflows and composite actions, GitLab CI files, the scripts, Makefiles, package-manager and registry configs, Terraform and Dockerfiles those jobs execute or build, and the credentials, runners, caches and artifacts they reach. Go deepest on three properties for every job:
- Trust: name who can cause it to run (fork PR, comment, push to any branch, schedule, maintainer) and every value or file they control that reaches a script, an action input, an install or a build.
- Privilege: name what the job holds while that input runs: secrets, token scopes, cloud identity, the runner host, caches and artifacts that later privileged or release jobs consume.
- Pinning: name every action, reusable workflow, image, package and remote script it pulls, and whether a ref someone else controls can change the code that runs.

Walk each job as an outsider would trigger it: a fork PR, an issue comment, a crafted branch name, a poisoned artifact or cache entry, a moved tag. When the change adds or alters no pipeline, build or image definition, say so plainly in the ledger.
