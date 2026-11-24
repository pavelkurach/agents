# token-claims-unchecked
langs: any

When: JWT, OIDC ID token, SAML assertion or other signed token decoded or verified, or its verification options changed.
Ask: Does the verifier pin the algorithm instead of reading it from the token header (`alg: none`, RS256-to-HS256 key confusion)? Are `aud` and `iss` checked against this service's own values, so a valid token minted for another service or tenant is rejected? Are `exp` and `nbf` enforced with bounded leeway? Is the key picked by `kid` only from the issuer's trusted key set, never from a URL in the token (`jku`, `x5u`)? Do claims come from an unverified decode (`verify_signature: False`, jsonwebtoken `decode` instead of `verify`)?
Report as: `<verify call>` accepts tokens `<with any audience / issuer / algorithm / expiry>`: a token issued for `<other service or tenant>` `<or forged with alg none>` authenticates here; fix: pin the algorithms, require and check `aud`, `iss` and `exp` against this service's values, and resolve keys only from the trusted JWKS.
