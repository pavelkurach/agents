# secret-compared-non-constant-time
langs: any

When: Token, API key, HMAC, signature or reset code compared with a caller-supplied value via `==`, `!=`, `equals`, `bytes.Equal` or a byte loop.
Ask: Is one side secret and the other attacker-controlled? Is a constant-time compare used — `hmac.compare_digest`, `crypto.timingSafeEqual` after a length check (it throws on unequal lengths), Go `subtle.ConstantTimeCompare` (`bytes.Equal` is not constant-time), Rust `subtle::ConstantTimeEq` — or is the token looked up by its hash instead?
Not when: the value goes through a password-verify function (`bcrypt.checkpw`, argon2 `verify`) · public identifiers.
Report as: `<compare>` exits at the first differing byte: an attacker can recover `<secret>` byte by byte from response timing; fix: `<constant-time compare>`, or store and look up a hash of the token.
