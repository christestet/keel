# users-service — the M6 exit criterion

This is the canonical "what Keel is for" program: a complete CRUD service with
zero external dependencies. It is aspirational until M6 — it uses stdlib modules
(http, sql, json, log, config) and post-Core features. Do not "fix" it to match
Core; Core grows to meet it.

**Aspirational features used (not yet in Core/spec):**

- `Uuid`, `Timestamp`, `Email` scalar types
- `log.info("msg", key: value)` named-argument structured logging
- `fn main() -> Result<Unit, Error>` union error return type

Deployment target:

    FROM scratch
    COPY ./users-svc /users-svc
    ENTRYPOINT ["/users-svc"]
