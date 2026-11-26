# build-secret-baked-into-image
langs: dockerfile

When: Dockerfile passes a token, key or credentials file into the build with `ARG`, `ENV`, `COPY` or a `RUN` that writes it, including when a later instruction deletes it.
Ask: Every layer keeps the files it wrote, so `RUN rm` in a later layer hides a file without removing it, and `ARG` and `ENV` values are recorded in the image config and history. Anyone who can pull the image reads them. Does the secret reach the final stage of a multi-stage build, or only a discarded builder stage? Is the build cache, which keeps intermediate layers, shared or exported? `docker/build-push-action` on a public repository attaches `mode=max` provenance by default, which records every build-arg value, builder stages included.
Not when: `RUN --mount=type=secret` or `--mount=type=ssh` · a secret file only in a builder stage whose layers are neither pushed nor exported with `--cache-to … mode=max`; an `ARG` there still lands in max-mode provenance.
Report as: `<instruction>` puts `<secret>` into `<layer / image config>` of `<image>`: everyone who can pull it reads the credential; fix: `RUN --mount=type=secret,id=<name>` (BuildKit) and rotate the exposed credential.
