# users-service — the M6 exit criterion

This is the canonical "what Keel is for" program: a complete CRUD service using
the M6 standard-library slice (`http`, `sql`, `json`, `log`, and `config`). The
M6 exit gate is reached: its SQLite behavior is locked by conformance cases 804
and 806.

It demonstrates:

- `Uuid`, `Timestamp`, `Email` scalar types
- `fn main() -> Result<Unit, Error>` boundary error propagation
- typed JSON, HTTP routing, SQL row mapping, and configuration loading

The source also contains `log.info("msg", key: value)`. The compiler currently
accepts it, but spec chapter 15 still marks structured log arguments as
aspirational and no conformance case locks their behavior. Do not treat that
call shape as stable yet.

Current build command:

```sh
target/release/keel build examples/users-service/main.keel --milestone M7
```

The intended deployment shape is:

    FROM scratch
    COPY ./users-svc /users-svc
    ENTRYPOINT ["/users-svc"]
