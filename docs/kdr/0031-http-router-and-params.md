# KDR-0031: `std.http` — Router and parameter extraction

- **Status:** proposed
- **Date:** 2026-06-19
- **Scope:** stdlib
- **Supersedes:** [KDR-0028](0028-http-server-surface.md)

## Decision

`std.http` is extended with a `Router` type for declarative route tables and
typed parameter extraction on `http.Request`. This KDR supersedes KDR-0028 and
replaces its Decision section entirely.

**Types**

- `http.Router` — compiler-known struct type. Constructed via struct literal
  syntax with route-pattern keys and handler function values:

  ```keel
  http.Router{
      "POST   /users":      fn(req) => handle_create(db, req),
      "GET    /users":      fn(req) => handle_list(db, req),
      "GET    /users/{id}": fn(req) => handle_get(db, req),
      "PATCH  /users/{id}": fn(req) => handle_update(db, req),
      "DELETE /users/{id}": fn(req) => handle_delete(db, req),
  }
  ```

  Each key is a string with the HTTP method (uppercase, whitespace-separated
  from the path) and a path pattern. Path patterns may contain `{name}`
  segments for parameter extraction. Each value is either a bare function name
  or a closure expression matching `fn(http.Request) -> http.Response`. Closures
  may capture variables from the enclosing scope (e.g. a database pool).

- `http.Request` — opaque compiler-known type with readable fields
  `body: String`, `method: String`, `path: String` and methods:

  ```text
  fn req.query(name: String) -> Option<String>
  fn req.header(name: String) -> Option<String>
  fn req.path_param<T>(name: String) -> Result<T, String>
  fn req.query_param<T>(name: String) -> Option<T>
  ```

  `path_param<T>` extracts the named path segment and parses it into type `T`.
  Returns `Err(message)` if the segment is absent or cannot be parsed — the
  error message is a human-readable string describing the failure. The error
  is caught via `catch`. `query_param<T>` extracts the first query-parameter
  value and parses it into `Option<T>`: `Some(parsed)` if present and valid,
  `None` if absent. The `??` operator provides a default when `None`.

- `http.Response` — compiler-known type with readable fields `status: Int` and
  `body: String`. Constructed only through the response-constructor functions.

- `http.Error` — compiler-known enum with one variant:
  `BindFailed(message: String)`.

**Handler model**

Handlers are functions (named or closures) with signature:

```text
fn <name>(req: http.Request) -> http.Response
```

`http.serve` accepts a `Router` value. The compiler validates each handler
value (name or closure) resolves to the correct signature. If any handler
does not match, the compiler emits `K1504`.

**`http.serve`**

```text
fn http.serve(port: Int, routes: http.Router) -> Result<Unit, http.Error>
```

`http.serve` is a compiler-known call. It blocks until the process is
terminated or a `BindFailed` error occurs. The port must be in the range
1–65535; out-of-range values are a `K1505` runtime panic.

**Response constructors**

All return `http.Response` with a fixed status code and the supplied body
(empty string for body-free responses):

| Constructor | Status |
|---|---|
| `http.ok(body: String) -> http.Response` | 200 |
| `http.created(body: String) -> http.Response` | 201 |
| `http.no_content() -> http.Response` | 204 |
| `http.bad_request(body: String) -> http.Response` | 400 |
| `http.not_found() -> http.Response` | 404 |
| `http.conflict(body: String) -> http.Response` | 409 |
| `http.internal_error(body: String) -> http.Response` | 500 |

All body arguments are UTF-8 strings. The backend does not apply HTML
escaping.

**Error conditions**

- `K1504` — invalid handler. A route value is not a Name resolving to
  `fn(http.Request) -> http.Response`. Compile-time error.
- `K1505` — invalid port. Runtime panic.

## Context

The users-service example (`examples/users-service/main.keel`) requires an
HTTP server with path-parameter routing and closure-based handler dispatch.
The original KDR-0028 single-handler model forced all routing into user code.
The design choices come from:

**The users-service pattern.** The example defines a `routes(db: sql.Pool)`
function that returns an `http.Router` with closure values capturing the db
pool: `fn(req) => handle_create(db, req)`. This is the natural way to pass
service state (database connections, configuration) to handlers without global
variables. The Router struct literal with closures is the M6 service pattern.

**Go's `net/http`** provides `http.HandleFunc(pattern, handler)` for
registration. Clean but stringly typed — the pattern and method are separate
arguments, and there is no compile-time check that the pattern matches the
handler's expectations.

**Express / Fastify** use method-specific router objects
(`router.get("/path", handler)`). Ergonomic but requires method-specific
registration and middleware chaining that is hard to typecheck.

**Axum (Rust)** provides type-safe routing via extractors. The closest analog
to Keel's approach but requires macros and a complex trait system. KDR-0004
prohibits macros. Keel gets the same compile-time safety through the
`path_param` method with type-parsing generics.

**`path_param<T>` typing.** The type parameter `T` controls how the raw
string segment is parsed. The return type is `Result<T, String>`: `Ok(parsed)`
on success, `Err(message)` on parse failure or absent segment. For example,
`path_param<String>` always succeeds (raw segment), while `path_param<Int>`
parses the segment as an integer and returns `Err("invalid integer: ...")` on
failure. This keeps the extraction call site concise:
`req.path_param<Uuid>("id")` instead of `req.path_param("id")` followed by
a manual parse. The `String` error type matches `http.bad_request(err)` for
the standard error-response pattern.

**Route pattern syntax.** `"GET /users/{id}"` is a single string with
method and path. The method is the first whitespace-delimited token. The path
may contain `{name}` segments for extraction. Static segments are matched
exactly. No wildcards, catch-all segments, or regex patterns in M6.

## Alternatives considered

- **Single-handler `http.serve(port, handler)`.** The original KDR-0028
  approach. Rejected: forces all routing into user code, making the common
  pattern (multi-endpoint service) verbose and error-prone.

- **`http.Handler` interface.** Would allow stateful handlers carrying a db
  pool as a field. Rejected for M6: requires a compiler-known interface type
  and one `impl` block per service. Closures provide the same state-carrying
  pattern more concisely.

- **Middleware chains.** Express-style `app.use(middleware)` is hard to
  typecheck and mixes concern ordering with business logic. Rejected.

- **Method-specific registration.** `router.get(path, handler)` /
  `router.post(path, handler)`. Rejected: the struct-literal form is more
  concise and declaration-ordered, matching Keel's deterministic semantics.

- **Wildcard and regex path patterns.** Rejected for M6: adds conflict
  resolution complexity and runtime matching cost. Static segments and
  `{name}` extraction cover the common case.

- **`req.query_param<T>` as a separate method from `req.query`.** They serve
  different purposes: `query` returns the raw `Option<String>` (no parsing),
  while `query_param<T>` parses the value into the target type and returns
  `Option<T>`. Both remain — `query` for raw access, `query_param<T>` for
  typed extraction.

## Consequences

- The `Router` struct literal with closure values is the standard M6 service
  pattern. Bare function names are also valid for handlers that need no
  captured state.
- Path parameter extraction is typed via generics returning `Result<T, String>`.
  The compiler resolves the type argument and generates the appropriate parser.
  Parse failures produce `Err(message)` caught via `catch`.
- The `http.Router` struct carries route patterns and handler references. The
  compiler generates the routing table at compile time; there is no runtime
  pattern-matching engine.
- The `fn main()` pattern in the users-service example passes a `Router` to
  `http.serve`. This is the standard M6 service pattern.
- Outbound HTTP calls are not in scope for M6.

## Reopening clause

1. Evidence from ≥3 real Keel services that the struct-literal Router form
   causes problems with dynamic route registration (e.g. plugin systems,
   runtime-loaded route tables), AND that a programmatic registration API
   measurably reduces boilerplate.
2. Evidence that typed `path_param<T>` extraction leads to a frequent bug
   class (e.g. parse failures at runtime for path segments that should have
   been validated at compile time), AND that a compile-time validation pass
   is feasible without `keel gen` integration.
3. Evidence that closure capture in Router handler values has measurable
   runtime overhead compared to bare function names, AND that the overhead
   matters for the target use case (high-throughput HTTP services).
