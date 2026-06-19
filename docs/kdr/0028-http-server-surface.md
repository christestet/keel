# KDR-0028: Typed HTTP server surface (`std.http`)

- **Status:** accepted
- **Date:** 2026-06-19
- **Scope:** stdlib

## Decision

`std.http` is a compiler-known, typed HTTP module. Its surface for M6 is:

**Types**

- `http.Request` — opaque compiler-known type with readable fields `body: String`,
  `method: String`, `path: String` and methods `query(name: String) -> Option<String>`
  and `header(name: String) -> Option<String>`.
- `http.Response` — compiler-known type with readable fields `status: Int` and
  `body: String`. Constructed only through the response-constructor functions; no
  user-written literals.
- `http.Error` — compiler-known enum with one variant: `BindFailed(message: String)`.

**Handler model**

The handler is a named function with signature:

```text
fn <name>(req: http.Request) -> http.Response
```

`http.serve` accepts its handler positionally as a bare function name, not as a
closure or first-class value. The compiler resolves the name, checks its signature,
and emits the Go server adapter. Function types are not added to the Core type
system. If the second argument to `http.serve` is not a Name that resolves to a
function with that signature, the compiler emits `K1504`.

**`http.serve`**

```text
fn http.serve(port: Int, handler: <fn(http.Request) -> http.Response>) -> Result<Unit, http.Error>
```

`http.serve` is a compiler-known call (not a user-callable function). It blocks
until the process is terminated or a `BindFailed` error occurs. The port must be a
positive integer in the range 1–65535; out-of-range values are a `K1505` runtime
panic. The Go backend binds on all interfaces.

**Response constructors**

All return `http.Response` with a fixed status code and the supplied body (empty
string for body-free responses):

| Constructor | Status |
|---|---|
| `http.ok(body: String) -> http.Response` | 200 |
| `http.created(body: String) -> http.Response` | 201 |
| `http.no_content() -> http.Response` | 204 |
| `http.bad_request(body: String) -> http.Response` | 400 |
| `http.not_found() -> http.Response` | 404 |
| `http.conflict(body: String) -> http.Response` | 409 |
| `http.internal_error(body: String) -> http.Response` | 500 |

All body arguments are UTF-8 strings. The backend does not apply HTML escaping.

## Context

The users-service example (`examples/users-service/main.keel`) requires an HTTP
server. The design choices come from observed costs in other languages:

**Go's `net/http`** relies on an `http.Handler` interface with a single method
`ServeHTTP(ResponseWriter, *Request)`. This is clean but requires mutation via
`ResponseWriter` rather than returning a value. Keel's response-as-value model is
easier to test and compose.

**Express/Koa/Fastify** use middleware chains where `req`, `res`, and `next`
are mutated in sequence. This is hard to typecheck statically and mixes concern
ordering with business logic.

**Axum (Rust)** provides type-safe routing via extractors but requires macros
and a complex trait system to make them ergonomic. KDR-0004 prohibits macros.

**Interface-based handler (considered)**: An `http.Handler` interface with
`fn handle(self, req: http.Request) -> http.Response` would allow stateful
handlers (carrying a db pool as a field) and reuse the existing impl system.
Rejected for the initial M6 surface because it requires a compiler-known
interface type, adds one `impl` block per service, and the state-carrying
pattern is better handled once closures land as a language feature. The
function-name approach is the minimal working model and can be superseded.

**Closure/lambda handler**: `fn(req) => dispatch(db, req)` is the cleanest
ergonomic form and what the aspirational users-service sketch uses. Rejected
for M6: function types are not in Core, adding them is a language change
requiring its own KDR. The function-name approach degrades gracefully: a
module-level handler function is always an option, and the demo service is
updated to use it.

**Path-parameter router**: `"GET /users/{id}"` pattern matching requires a
router that extracts named segments. Rejected for M6: requires a runtime
pattern-matching engine and raises questions about conflict resolution and
registration order. Positional segment extraction (`req.path_segment(index)`)
is the minimal tool; a pattern router is post-M6 once the common use-case is
demonstrated.

## Alternatives considered

- **`http.Handler` interface, struct-based handler with state.** See context above.
- **`http.serve` accepts a closure value.** Blocked by absence of function types
  in Core.
- **Body as `Bytes` type.** Rejected for M6: `Bytes` is not yet in the type system.
  Bodies are UTF-8 strings in the initial surface; non-UTF-8 responses are
  post-M6.
- **Response as a method-call chain (builder pattern).** Rejected: KDR-0004 and
  the absence of method-chaining syntax on stdlib types.
- **Separate client and server modules.** Deferred: `http.Client` for outbound
  requests is post-M6, blocked on confirmed use-case and timeout/context
  propagation design.
- **`http.serve` returns `Unit` and panics on bind failure.** Rejected: panics on
  user-visible conditions violate the hard rule that malformed configuration
  produces `Result`, not a crash.

## Consequences

- The `examples/users-service/main.keel` demo must be updated: `http.Router`
  with closure syntax is replaced by a module-level dispatch function.
- Path parameters require positional extraction; named extraction is post-M6.
- Stateful handlers (carrying a db pool) are not ergonomic until closures land;
  the interim pattern is a module-level function that calls handler functions
  passing the pool as an argument (requires the pool to be available in scope).
- Outbound HTTP calls are not in scope for M6.

## Reopening clause

1. Evidence from ≥3 real Keel services that the named-function handler model
   requires unsafe workarounds (global variables, thread-locals) to carry service
   state, AND that an interface-based model measurably reduces that code; or
2. Addition of function types to Core, at which point the handler argument
   becomes a first-class value and the `K1504` guard generalizes.
