# pooled-client-built-per-call
langs: any

When: Pooled client (HTTP session, DB engine or pool, cloud SDK client, gRPC channel) constructed inside a per-request, per-item or per-task function.
Ask: Is it built once per process or once per call? Per call, each one opens fresh connections and TLS handshakes, and engines or sessions left unclosed leak their pools: `requests.Session`, `httpx.Client`/`AsyncClient`, `aiohttp.ClientSession`, SQLAlchemy `create_engine`, `boto3.client`, `reqwest::Client::new()`, a gRPC channel, a Go `http.Client` with its own `Transport`. Does the codebase already hold a shared instance for this dependency?
Not when: a short-lived CLI or script that makes one call.
Report as: `<client>` is built per `<call>` at `<site>`: no connection reuse, a handshake per call, and `<leaked pools / fd or ephemeral-port exhaustion>` under load; fix: build it once at startup (module level or a DI singleton), share it, and close it at shutdown.
