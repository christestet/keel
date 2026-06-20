# 15 — Standard library core

This chapter is **normative**. It specifies the standard-library surface needed
to complete the deadline and cancellation semantics in
[`09-concurrency.md`](09-concurrency.md), the typed JSON boundary governed
by [`KDR-0027`](../kdr/0027-json-boundary-mapping.md), the HTTP server with
Router and parameter extraction governed by
[`KDR-0031`](../kdr/0031-http-router-and-params.md), the SQL database access
surface governed by [`KDR-0029`](../kdr/0029-sql-database-access.md), and the
configuration loading surface governed by
[`KDR-0030`](../kdr/0030-config-loading-surface.md).

## 15.1 The `std.time` module

`use std.time` imports the `time` module. It exposes one opaque value type,
`time.Duration`, and two constructors:

```keel
use std.time

fn main() {
    let short = time.milliseconds(250)
    let long = time.seconds(2)
}
```

The signatures are:

```text
fn time.milliseconds(value: Int) -> time.Duration
fn time.seconds(value: Int) -> time.Duration
```

A duration is a non-negative number of nanoseconds. `milliseconds` multiplies
its argument by 1,000,000 and `seconds` multiplies its argument by
1,000,000,000. A negative argument panics with `K1501`. Multiplication outside
the `Int` range panics with the existing integer-overflow code `K0203`.
`time.Duration` has no public fields and no arithmetic operators.

## 15.2 Relative scope deadlines

The `deadline` argument of a `scope` has type `time.Duration`:

```keel
use std.time

fn work() -> Unit {}

fn main() {
    scope(deadline: time.seconds(2)) {
        spawn work()
    }
}
```

The duration is measured from entry into that scope using a monotonic clock.
Wall-clock changes do not alter an active deadline. A zero duration is already
expired when the scope is entered.

Passing any other type as `deadline` is `K1502`. The diagnostic's primary span
is the deadline expression.

Deadline propagation, nested-deadline tightening, fail-fast behavior, and the
`Cancelled` error are defined by [`09-concurrency.md`](09-concurrency.md)
§§9.4–9.6. This chapter only fixes the concrete deadline value type and clock
semantics.

## 15.3 Cancellation checkpoints

The module exposes a cancellation-aware sleep:

```text
fn time.sleep(duration: time.Duration) -> Result<Unit, Cancelled>
```

`time.sleep` returns `Ok(())` after the duration elapses. If the ambient scope
is cancelled first, it returns `Err(Cancelled)`. A zero-duration sleep is still
a cancellation checkpoint: it returns `Err(Cancelled)` when cancellation is
already pending and `Ok(())` otherwise.

Compute-bound code uses the built-in checkpoint:

```text
fn check_cancel() -> Result<Unit, Cancelled>
```

`check_cancel()` returns `Err(Cancelled)` when cancellation is pending in the
ambient scope and `Ok(())` otherwise. Outside a `scope`, no cancellation is
pending, so it returns `Ok(())`. Propagation is explicit through the existing
`?` operator:

```keel
fn work() -> Result<Unit, Cancelled> {
    check_cancel()?
    Ok(())
}
```

Cancellation remains an ordinary value and never a panic.

## 15.4 Error conditions

- **`K1501` — negative duration.** `time.milliseconds` or `time.seconds` is
  evaluated with a negative argument. This is a runtime panic.
- **`K1502` — invalid deadline type.** A `scope(deadline: expression)` uses an
  expression whose static type is not `time.Duration`. This is a compile-time
  error.

Both codes are permanent. Their message text may improve without changing the
code.

## 15.5 Planned time conformance cases

These cases land in the following conformance-only PR. This spec PR does not
add or alter executable cases.

| Case | Kind | Asserts |
|---|---|---|
| `716-scope-deadline-cancelled` | accept | a deadline cancels a sleeping task and can be handled as `Cancelled` |
| `717-nested-deadline-tightens` | accept | a shorter nested deadline wins over its parent deadline |
| `718-check-cancel-outside-scope` | accept | the explicit checkpoint returns `Ok(())` without an ambient scope |
| `719-negative-duration` | reject `K1501` | duration constructors reject negative values |
| `720-deadline-type` | reject `K1502` | `deadline` requires `time.Duration` |
| `721-duration-overflow` | reject `K0203` | duration conversion preserves the integer-overflow contract |
| `722-zero-deadline-cancelled` | accept | a zero deadline is expired on scope entry |
| `723-zero-sleep-outside-scope` | accept | zero-duration sleep succeeds without pending cancellation |

## 15.6 Dependencies

- [`09-concurrency.md`](09-concurrency.md) — scope, deadline propagation, and
  cancellation semantics.
- [`KDR-0002`](../kdr/0002-no-async-await.md) — ambient structured
  concurrency without function coloring.
- [`KDR-0005`](../kdr/0005-no-exceptions.md) — cancellation travels through
  `Result`, not exceptions or panics.
- [`KDR-0026`](../kdr/0026-structured-concurrency-mechanism.md) — join,
  fail-fast, deadline, and cooperative-checkpoint mechanism.

## 15.7 The `std.json` module

`use std.json` imports the compiler-known `json` module. It exposes these
operations:

```text
fn json.parse<T>(input: String) -> Result<T, json.Error>
fn json.parse<T>(input: String, mode: .tolerant) -> Result<T, json.Error>
fn json.write<T>(value: T) -> Result<String, json.Error>
```

`json.parse` requires exactly one explicit concrete type argument. `json.write`
infers `T` from its value argument. Both calls require a JSON-representable type
as defined in §15.8; using another type is the compile-time error `K1503`, with
the type argument or value expression as the primary span.

The second `parse` spelling is a compiler-known named argument, not a general
named-argument or enum-shorthand feature. `.tolerant` is its only valid value.
Strict parsing is the default. Tolerant parsing changes only unknown-field
handling as specified in §15.10.

## 15.8 Representable Keel types

The following types are JSON-representable when every recursively contained
type is also representable:

- `Bool`, `String`, `Char`, `Int`, and `Float`;
- `Option<T>` and `List<T>`;
- `Map<String, T>`; no other map key type is representable;
- concrete structs and enums; and
- standard-library scalar types whose own normative specification declares a
  JSON wire representation.

`Unit`, interfaces, function values, task handles, unconstrained type
parameters, and all other types are not JSON-representable. There is no public
dynamic JSON tree, serializer interface, annotation, rename directive, custom
codec registry, or implicit string conversion.

`Char` maps to a JSON string containing exactly one Unicode scalar value.
`Option<T>` maps `None` to JSON `null` and `Some(value)` to the representation of
`value`. Parsing `null` into any non-`Option` type is a type mismatch.

## 15.9 Struct and enum mapping

A hand-written struct maps to a JSON object. Field names match the Keel source
names exactly and case-sensitively. Every non-`Option` field is required. An
absent `Option<T>` field becomes `None`; a present field is parsed normally.
Source-level struct defaults are not wire defaults and do not make a field
optional while parsing.

A hand-written enum has one adjacent-tagged representation. Both object fields
are required:

```json
{"variant":"Active","fields":{}}
{"variant":"Suspended","fields":{"reason":"maintenance"}}
```

`variant` is exactly the Keel variant name. `fields` is an object whose names
and required/optional rules are the same as struct fields. A unit variant
requires an empty `fields` object. Unknown variants are type mismatches.

Compiler-owned schema metadata produced by `keel gen` may select wire names and
an enum representation required by that schema. Such metadata is not
user-expressible syntax. It is outside the M6 surface and does not change the
mapping above for hand-written types.

## 15.10 Strictness and tolerant parsing

Strict `json.parse` rejects all of the following with `json.Error`:

- malformed JSON, invalid UTF-8, invalid escapes, or trailing non-whitespace;
- duplicate names in any object;
- an unknown struct, enum-envelope, or enum-payload field;
- a missing required field;
- a JSON value of the wrong type; and
- a number outside the target type's range.

`mode: .tolerant` changes exactly the unknown-field rule. Unknown fields are
ignored and one structured `schema_drift` event is emitted for each ignored
field, including its JSON path. Tolerant parsing still rejects malformed input,
duplicates, missing fields, wrong types, and out-of-range numbers. If no
telemetry sink is configured, event emission has no program-visible effect and
does not make parsing fail.

Object duplicates are detected from the input token stream before conversion;
an implementation may not inherit a backend's last-key-wins map behavior.

## 15.11 Numeric mapping

`Int` accepts only a JSON number token with no decimal point and no exponent,
whose mathematical value is in the inclusive signed 64-bit range. It does not
accept a numeric string, boolean, `1.0`, or `1e0`.

`Float` accepts a JSON number whose value is finite and representable by Keel
`Float`. JSON strings and the non-standard tokens `NaN`, `Infinity`, and
`-Infinity` are never numbers. `json.write` returns `json.NonFinite` when asked
to encode a non-finite `Float`.

## 15.12 Deterministic writing

`json.write` emits valid UTF-8 with no insignificant whitespace. Struct fields
use declaration order. Enum envelope fields are written as `variant`, then
`fields`; payload fields use declaration order. Map entries are sorted by key in
Unicode scalar-value order. Integers use canonical decimal notation. Floats use
the shortest decimal representation that parses to the same `Float` value.

The writer escapes JSON control characters, quotation mark, and reverse solidus
as required by JSON. It does not apply backend-specific HTML escaping. For the
same Keel value, the output bytes are identical across runs and backends.

## 15.13 `json.Error`

JSON failures are values with this public shape:

```keel
enum json.Error {
    Syntax(offset: Int),
    TypeMismatch(path: String, expected: String),
    MissingField(path: String),
    UnknownField(path: String),
    DuplicateField(path: String),
    OutOfRange(path: String),
    NonFinite(path: String),
}
```

Offsets are zero-based UTF-8 byte offsets. Paths use `$` for the root,
`.field_name` for an object field, and `[index]` for a list element. The first
error in input order is returned. When a missing-field check occurs after an
object has been read, declaration order determines which missing field is
reported. Message text is not part of the contract.

## 15.14 JSON error conditions

- **`K1503` — unsupported JSON target.** `json.parse<T>` or `json.write(value)`
  uses a type not included in §15.8. This is a compile-time error.

`json.Error` variants are runtime values, not compiler diagnostics. `K1503` is
permanent; its message text may improve without changing the code.

## 15.15 Planned JSON conformance cases

These cases land in the following conformance-only PR. This spec change does not
add or alter executable cases.

| Case | Kind | Asserts |
|---|---|---|
| `724-json-parse-struct` | accept | a strict object parses into a concrete struct |
| `725-json-write-struct-order` | accept | struct output is compact and follows declaration order |
| `726-json-missing-required` | accept | a missing required field returns `MissingField` |
| `727-json-option-absent` | accept | an absent optional field becomes `None` |
| `728-json-unknown-strict` | accept | strict parsing returns `UnknownField` |
| `729-json-unknown-tolerant` | accept | tolerant parsing ignores only an unknown field |
| `730-json-duplicate-field` | accept | duplicate names return `DuplicateField` |
| `731-json-int-out-of-range` | accept | integer overflow returns `OutOfRange` |
| `732-json-int-token-strict` | accept | fraction/exponent syntax is not accepted as `Int` |
| `733-json-enum-roundtrip` | accept | enums use the uniform adjacent-tagged shape |
| `734-json-trailing-input` | accept | trailing non-whitespace returns `Syntax` |
| `735-json-unsupported-target` | reject `K1503` | an unsupported target is rejected statically |

## 15.16 JSON dependencies

- [`KDR-0004`](../kdr/0004-no-macros.md) — compiler-known derivation without
  reflection, annotations, or user metaprogramming.
- [`KDR-0015`](../kdr/0015-boundary-doctrine.md) — typed boundaries, strict
  default, explicit and observable tolerance.
- [`KDR-0027`](../kdr/0027-json-boundary-mapping.md) — concrete JSON mapping,
  determinism, and rejected compatibility defaults.

---

## 15.17 The `std.http` module

`use std.http` imports the compiler-known `http` module. It exposes an HTTP
server with response-as-value semantics and a `Router` type for declarative
route tables.

### 15.17.1 `http.Router`

`http.Router` is a compiler-known struct type. It is constructed via struct
literal syntax with route-pattern keys and handler function values:

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
segments for parameter extraction. Static segments are matched exactly. Each
value is either a bare function name or a closure expression matching
`fn(http.Request) -> http.Response`. Closures may capture variables from the
enclosing scope (e.g. a database pool).

### 15.17.2 `http.serve`

```text
fn http.serve(port: Int, routes: http.Router) -> Result<Unit, http.Error>
```

The `routes` argument must be an `http.Router` value whose handler values
all resolve to functions with signature `fn(http.Request) -> http.Response`.
If any handler does not match, the compiler emits `K1504`.
`http.serve` blocks until the process is terminated or a bind error occurs.
It is a compile-time error if `http.serve` is called without `use std.http`.

## 15.18 `http.Request`

`http.Request` is an opaque compiler-known type. It has three readable fields:

```text
body:   String   — the full request body as a UTF-8 string
method: String   — the HTTP method in upper case ("GET", "POST", …)
path:   String   — the URL path component without the query string
```

Four methods extract values:

```text
fn req.query(name: String) -> Option<String>          — first query-parameter value for name
fn req.header(name: String) -> Option<String>         — first header value for name (canonical form)
fn req.path_param<T>(name: String) -> Result<T, String> — named path segment parsed into type T
fn req.query_param<T>(name: String) -> Option<T>      — query-parameter parsed into Option<T>
```

`path_param<T>` extracts the value of a named segment matching a `{name}`
pattern in the route that dispatched to this handler, then parses it into
type `T`. Returns `Ok(parsed)` on success, `Err(message)` on parse failure
or absent segment — the error message is a human-readable string. For
example, given the route `"GET /users/{id}"`, a request to `/users/42` makes
`req.path_param<Uuid>("id")` return `Ok(Uuid)`. If the segment is absent or
cannot be parsed into `T`, the call returns `Err(message)` which can be
caught via `catch`.

`query_param<T>` extracts the first query-parameter value for `name` and
parses it into `Option<T>`: `Some(parsed)` if present and parseable, `None`
if absent. The `??` operator provides a default when `None`. For example:
`req.query_param<Int>("limit") ?? 50`.

`http.Request` cannot be constructed by user code in M6. It is created by the
server runtime and passed to the handler for each incoming request.

## 15.19 `http.Response`

`http.Response` is a compiler-known type. It has two readable fields:

```text
status: Int    — the HTTP status code
body:   String — the response body
```

`http.Response` is constructed only through the response-constructor functions in
§15.20. User code cannot write `http.Response{ … }` literals.

## 15.20 Response constructors

Each constructor returns an `http.Response` with a fixed status code and the
supplied body (or an empty body for body-free responses):

```text
fn http.ok(body: String) -> http.Response             — 200 OK
fn http.created(body: String) -> http.Response        — 201 Created
fn http.no_content() -> http.Response                 — 204 No Content
fn http.bad_request(body: String) -> http.Response    — 400 Bad Request
fn http.not_found() -> http.Response                  — 404 Not Found
fn http.conflict(body: String) -> http.Response       — 409 Conflict
fn http.internal_error(body: String) -> http.Response — 500 Internal Server Error
```

Body arguments are UTF-8 strings. The backend does not apply HTML escaping to
the body.

## 15.21 `http.Error`

HTTP server failures are values with this public shape:

```keel
enum http.Error {
    BindFailed(message: String),
}
```

`BindFailed` is returned by `http.serve` when the port cannot be bound (already
in use, insufficient permissions, or invalid port number at runtime).

## 15.22 HTTP error conditions

- **`K1504` — invalid HTTP handler.** A route value in the `http.Router` is not
  a Name or closure that resolves to a function with signature
  `fn(http.Request) -> http.Response`. This is a compile-time error; the primary
  span is the offending route value.
- **`K1505` — invalid HTTP port.** The port argument to `http.serve` is outside
  the range 1–65535 at runtime. This is a runtime panic.

Both codes are permanent. Message text may improve without changing the code.

## 15.23 Planned HTTP conformance cases

| Case | Kind | Asserts |
|---|---|---|
| `736-http-ok-response` | accept | `http.ok` sets status 200 and the supplied body |
| `737-http-created-response` | accept | `http.created` sets status 201 |
| `738-http-no-content-response` | accept | `http.no_content` sets status 204 and empty body |
| `739-http-bad-request-response` | accept | `http.bad_request` sets status 400 |
| `740-http-not-found-response` | accept | `http.not_found` sets status 404 and empty body |
| `741-http-conflict-response` | accept | `http.conflict` sets status 409 |
| `742-http-internal-error-response` | accept | `http.internal_error` sets status 500 |
| `743-http-response-body` | accept | response body is the string passed to the constructor |
| `744-http-serve-compiles` | build | `http.serve` with a valid `Router` compiles |
| `745-http-invalid-handler` | reject `K1504` | a non-handler route value is rejected statically |
| `767-http-router-closure-compiles` | build | `http.Router` with closure handler values compiles |
| `768-http-path-param` | build | `req.path_param<T>(name)` extracts and parses the named segment |
| `769-http-query-param` | build | `req.query_param<T>(name)` extracts and parses query-parameter |

## 15.24 HTTP dependencies

- [`KDR-0005`](../kdr/0005-no-exceptions.md) — bind failures are `Result`, not panics.
- [`KDR-0015`](../kdr/0015-boundary-doctrine.md) — typed request/response boundary.
- [`KDR-0031`](../kdr/0031-http-router-and-params.md) — Router type, handler model, path parameter extraction.

## 15.25 The `std.log` module

`use std.log` imports the compiler-known `log` module. It exposes three
level-based logging functions:

```text
fn log.info(message: String) -> Unit
fn log.warn(message: String) -> Unit
fn log.error(message: String) -> Unit
```

Each function writes its message to stdout, prefixed with the level in square
brackets and a space:

| Call | Output |
|---|---|
| `log.info("started")` | `[info] started` |
| `log.warn("slow query")` | `[warn] slow query` |
| `log.error("disk full")` | `[error] disk full` |

The level prefix is lower-case ASCII. A trailing newline is added. The backend
does not apply HTML escaping or any transformation to the message text.

Each function accepts exactly one argument of type `String`. Passing any other
type — or the wrong number of arguments — is a type error caught by normal
function call checking. There are no log-level filtering, structured context,
configurable output, or formatting features in M6.

The users-service example uses `log.info("listening", port: cfg.port)` with a
named-argument form for structured context. This is aspirational — named
arguments and structured logging are not part of the M6 log surface and
require their own KDR.

## 15.26 Planned log conformance cases

| Case | Kind | Asserts |
|---|---|---|
| `746-log-info-output` | accept | `log.info("hello")` writes `[info] hello` to stdout |
| `747-log-warn-output` | accept | `log.warn("careful")` writes `[warn] careful` |
| `748-log-error-output` | accept | `log.error("fail")` writes `[error] fail` |

## 15.27 Log dependencies

None for the M6 surface. The module uses the existing `print` output channel
and no new capabilities.

---

## 15.28 The `std.sql` module

`use std.sql` imports the compiler-known `sql` module. It exposes a database
connection pool and typed row mapping for SQL operations.

### 15.28.1 Connection

A pool is created only through `sql.connect`:

```text
fn sql.connect(connectionString: String) -> Result<sql.Pool, sql.Error>
```

`sql.connect` parses the connection string and opens a pool backed by the
Go `database/sql` connection pool (default max open = 25, max idle = 2). The
connection string format is driver-specific (database/sql-compatible
DataSourceName). The pool is lazy — no connection is established until the
first query.

`sql.Pool` is an opaque compiler-known type. It cannot be constructed by user
code and has no public fields.

### 15.28.2 Query execution and migrations

A pool provides four methods. Query methods accept a plain SQL string with
named parameters in the `$1`, `$2`, … style (Go-style positional parameters):

```text
fn pool.query(sqlStatement: String) -> Result<QueryResult, sql.Error>
fn pool.query_one(sqlStatement: String) -> Result<Row, sql.Error>
fn pool.exec(sqlStatement: String) -> Result<Int, sql.Error>
fn pool.migrate(migrationStatements: String) -> Result<Unit, sql.Error>
```

`query` returns a `QueryResult` carrying the full result set. The user iterates
with `result.next() -> Option<Row>` or maps to structs via `result.map(f) ->
RowMapper<T>` (see §15.28.3).

`query_one` returns a single `Row`. If zero rows match, the result is
`Err(NoRows)`. If more than one row matches, the result is
`Err(QueryFailed(...))`.

`exec` runs the statement and returns the number of rows affected by the
command as an `Int` (0 for non-SELECT queries).

`pool.migrate` runs all SQL statements in `migrationStatements` sequentially,
separated by semicolons. It returns on the first SQL error. Partial
execution of earlier statements is not retried. The user is responsible for
idempotent statements (e.g. `CREATE TABLE IF NOT EXISTS`) when running
migrations multiple times.

### 15.28.3 Row mapping — the `FromRow` function

A `QueryResult` provides two ways to consume rows:

**Iterator:**

```text
fn result.next() -> Option<Row>
```

Returns the next `Row` or `None` when iteration is exhausted. Each call
advances the iterator. The internal Go `*Rows` handle is closed automatically
after `None` and on garbage collection.

**Mapping to a struct:**

```text
fn result.map(f: <fn(Row) -> T>) -> RowMapper<T>
```

The `f` argument must be a bare function name that resolves to a function
with a signature matching `fn(sql.Row) -> TargetStruct`, where `TargetStruct`
is inferred from the function's return type. If no such function exists or the
signature does not match, the compiler emits `K1506`.

`map` returns a compiler-known `RowMapper<T>` value. It provides:

```text
fn mapper.collect() -> Result<List<T>, sql.Error>
```

`collect()` consumes all remaining rows, calling `f` once per row, and returns
the list of results.

Row field types are matched positionally against the Go column type registry.
For M6, the mapping is:

| Go column type | Keel field type |
|---|---|
| `int64` | `Int` |
| `int32`, `int16`, `int8`, `smallint`, `bigint` | `Int` |
| `text`, `varchar(n)`, `char(n)`, `uuid` | `String` |
| `boolean`, `bool` | `Bool` |
| `double precision`, `float8` | `Float` |
| `date`, `timestamp`, `timestamptz` | `Timestamp` |
| `time`, `timetz` | `Time` |
| `bytea` | `Bytes` |

When a row's column type does not match the target struct field type and there
is no registered conversion, `collect` returns `Err(QueryFailed(...))`. The
compiler does not validate column-to-field alignment against the query itself
in M6 — the mapping is purely positional and based on the returned row's column
types.

### 15.28.4 `sql.Error`

SQL failures are values with this public shape:

```keel
enum sql.Error {
    ConnectionFailed(message: String),
    QueryFailed(message: String),
    NoRows,
    UniqueViolation(message: String),
    MigrationFailed(message: String),
}
```

`ConnectionFailed` is returned by `sql.connect`. `QueryFailed` covers general
query errors (syntax errors, constraint violations, type mismatches) except when
a more specific variant applies. `NoRows` is returned by `query_one` when
zero rows match. `UniqueViolation` is returned when a unique constraint is
violated; the `message` field contains the database-level detail.
`MigrationFailed` is returned by `pool.migrate` on the first statement error.

### 15.28.5 SQL error conditions

- **`K1506` — invalid FromRow function.** `result.map(f)` uses a `f` that does
  not resolve to a function with signature `fn(sql.Row) -> TargetStruct`, or the
  same `f` is mapped to multiple struct types in a single `collect` call. This is
  a compile-time error; the primary span is the `f` argument in the `map` call.

`sql.Error` variants are runtime values, not compiler diagnostics. `K1506` is
permanent; its message text may improve without changing the code.

### 15.29 Planned SQL conformance cases

| Case | Kind | Asserts |
|---|---|---|
| `770-sql-connect` | build | `sql.connect` with connection string compiles |
| `771-sql-migrate` | build | `pool.migrate` with SQL statement compiles |
| `772-sql-query` | build | `pool.query` with SQL statement compiles |
| `773-sql-query-one` | build | `pool.query_one` with SQL statement compiles |
| `774-sql-exec` | build | `pool.exec` with SQL statement compiles |
| `775-sql-row-mapping` | build | `result.map(f).collect()` with `FromRow` function compiles |
| `749-sql-connect-fail` | accept | `sql.connect` with invalid string returns `ConnectionFailed` |
| `750-sql-query-return` | accept | `query` with valid statement returns a `QueryResult` |
| `751-sql-exec-affected` | accept | `exec` returns row count as `Int` |
| `752-sql-query-one-single` | accept | `query_one` returns a single row for matching query |
| `753-sql-query-one-no-rows` | accept | `query_one` returns `Err(NoRows)` for no-match |
| `754-sql-query-one-multiple` | accept | `query_one` returns `Err(QueryFailed)` for multi-match |
| `755-sql-pool-migrate-fail` | accept | `pool.migrate` with invalid SQL returns `MigrationFailed` |
| `756-sql-map-invalid-fn` | reject `K1506` | `map` with wrong-signature function is rejected |

### 15.30 SQL dependencies

- [`KDR-0005`](../kdr/0005-no-exceptions.md) — SQL failures are `Result`, not panics.
- [`KDR-0015`](../kdr/0015-boundary-doctrine.md) — typed row-to-struct boundary.
- [`KDR-0029`](../kdr/0029-sql-database-access.md) — pool model, `FromRow` function convention, rejected alternatives.

## 15.31 The `std.config` module

`use std.config` imports the compiler-known `config` module. It exposes a
typed, environment-variable-based configuration loader.

### 15.31.1 Loading configuration

```text
fn config.load<T>() -> Result<T, config.Error>
```

`config.load<T>` reads all fields of the struct type `T` from environment
variables. The type argument `T` must be a named struct declared at module
scope whose fields are all loadable. If `T` is not a named struct or contains
a field type that cannot be loaded from an environment variable, the compiler
emits `K1507`.

### 15.31.2 Env var name mapping

For a struct field named `f`, the corresponding environment variable name is
the uppercase form of `f`. Under score characters are preserved:

| Field name | Env var |
|---|---|
| `database_url` | `DATABASE_URL` |
| `port` | `PORT` |
| `retry_count` | `RETRY_COUNT` |
| `cache_ttl_seconds` | `CACHE_TTL_SECONDS` |

The mapping is `snake_case` → `UPPER_SNAKE_CASE`. No other transformations are
supported. For env var names that cannot follow this convention, the field name
must match the desired env var name exactly.

### 15.31.3 Parse rules by field type

- **`String`**: any non-empty env var value. An empty value is a valid string.
  Missing env var (not set at all) returns `MissingEnvVar`.
- **`Int`**: a digit sequence, optionally preceded by `-`. Returns
  `ParseError` on non-numeric content or out-of-range `Int64`.
- **`Float`**: a float string (`"3.14"`, `"-0.5"`, `"1e3"`). Returns
  `ParseError` on invalid format or non-finite result.
- **`Bool`**: accepts `"true"`, `"1"`, `"yes"`, `"on"` (case-insensitive) for
  `true`; `"false"`, `"0"`, `"no"`, `"off"` for `false`. Any other value is a
  `ParseError`.
- **`Option<T>`**: if the env var is set and non-empty, the value is parsed into
  `T` and returned as `Some(value)`. If the env var is set to an empty string
  or is not set, returns `None`. On parse failure, returns `ParseError`.
- **`Secret`**: behaves like `String` but signals missing data as
  `MissingSecret` instead of `MissingEnvVar`.

### 15.31.4 Struct defaults

Fields may carry a default value in the struct declaration:

```keel
struct AppConfig {
    database_url: Secret,
    port: Int = 8080,
}
```

When a field has a default, the env var is optional — the field is treated
like `Option<T>` for the purpose of missing-value handling. If the env var is
not set (or empty), the default is used instead.

### 15.31.5 `Secret`

`Secret` is a compiler-known struct in the `config` namespace:

```keel
struct Secret {
    value: String,
}
```

`Secret` wraps sensitive string data. It is distinct from `String` at the type
level. `json.write(secret)` does not include the actual value — the writer
produces `"<secret>"` instead of the secret's content. When `config.load`
encounters a missing env var for a `Secret` field, it returns
`MissingSecret(field_name)` rather than `MissingEnvVar`.

### 15.31.6 `config.Error`

Configuration failures are values with this public shape:

```keel
enum config.Error {
    MissingEnvVar(field_name: String),
    MissingSecret(field_name: String),
    ParseError(field_name: String, type: String, message: String),
    InvalidStructType(type_name: String),
}
```

`MissingEnvVar` is returned when a non-option, non-default field's env var is
not set. `MissingSecret` is the same for `Secret` fields. `ParseError` is
returned when an env var value cannot be parsed into the target type.
`InvalidStructType` is returned when `load<T>` is called with a non-loadable
type.

### 15.31.7 Config error conditions

- **`K1507` — unparseable config target.** `config.load<T>` is called with a
  type `T` that is not a named struct, or `T` contains a field type that cannot be
  loaded from an environment variable. This is a compile-time error; the primary
  span is the type argument or target expression.

`config.Error` variants are runtime values, not compiler diagnostics. `K1507` is
permanent; its message text may improve without changing the code.

### 15.32 Planned config conformance cases

| Case | Kind | Asserts |
|---|---|---|
| `776-config-load` | build | `config.load<T>()` with named struct compiles |
| `777-config-secret` | build | `Secret` field with `unwrap()` compiles |
| `778-config-missing` | build | `config.load<T>()` with required fields compiles |
| `757-config-load-string` | accept | `load<T>` with `String` field reads env var value |
| `758-config-load-int` | accept | `load<T>` with `Int` field parses numeric value |
| `759-config-load-bool` | accept | `load<T>` with `Bool` parses `true`/`false` variants |
| `760-config-load-default` | accept | missing env var uses struct default |
| `761-config-load-missing` | reject | `load<T>` with required field and no env var returns `MissingEnvVar` |
| `762-config-load-parse-fail` | accept | non-numeric value for `Int` field returns `ParseError` |
| `763-config-load-option-absent` | accept | missing env var for `Option<T>` returns `None` |
| `764-config-load-optional-default` | accept | missing env var + `Option<T>` with default returns `None` |
| `765-config-secret-missing` | accept | missing `Secret` field returns `MissingSecret` |
| `766-config-unsupported-type` | reject `K1507` | non-struct target is rejected statically |

### 15.33 Config dependencies

- [`KDR-0004`](../kdr/0004-no-macros.md) — no annotations for custom env var names.
- [`KDR-0005`](../kdr/0005-no-exceptions.md) — config failures are `Result`, not panics.
- [`KDR-0015`](../kdr/0015-boundary-doctrine.md) — typed config boundary with `Secret`.
- [`KDR-0030`](../kdr/0030-config-loading-surface.md) — env var mapping, parse rules, rejected alternatives.
