# server-side-fetch-of-input-url
langs: any

When: Server fetches a URL or host that a client or tenant supplies: webhook target, URL preview, import from URL, image or PDF renderer.
Ask: Is the destination checked after DNS resolution against an allowlist, rejecting loopback, private and link-local ranges (`169.254.169.254` metadata) and their IPv6 forms? Does the connection go to the address that was checked (DNS rebinding)? Does the client follow redirects without re-checking each hop?
Not when: the host comes from server configuration.
Report as: `<fetch>` lets `<caller>` make the server request `<internal target>`: cloud credentials and internal services become reachable from outside; fix: an allowlist, resolve once and connect to the pinned address, block private ranges, re-check every redirect, or route through an egress proxy.
