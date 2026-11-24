# outbound-call-without-timeout
langs: any

When: Call to another process (HTTP, RPC, DB, cache, queue, external API) added or changed, or its client built.
Ask: What bounds the wait? Read the call arguments and the client construction for connect and read timeouts or a deadline. Defaults often wait forever (`requests`, Go `http.DefaultClient`, `reqwest`, `fetch`) or for minutes (`aiohttp`, 5 min total).
Not when: a caller-level deadline covers it (`asyncio.timeout`, `context.WithTimeout`) · a stream with its own idle timeout.
Report as: `<call>` has no timeout: when `<dependency>` hangs, workers park on it until the pool drains and the service stops; fix: timeouts on the client, or a propagated deadline.
