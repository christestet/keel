# Standard library reference

This is the user-facing reference for the M6 standard-library slice. Exact
semantics and error conditions live in
[spec chapter 15](spec/15-stdlib-core.md); conformance cases 716–806 define what
the current compiler actually supports.

Every module is compiler-known and imported with `use std.<name>`. Packages with
a manifest must declare the capabilities listed below.

| Module | Required capabilities |
|---|---|
| `std.time` | none |
| `std.json` | none |
| `std.log` | none |
| `std.config` | `env` |
| `std.http` | `net` |
| `std.sql` | `net`, `fs` |

## `std.time`

```text
fn time.milliseconds(value: Int) -> time.Duration
fn time.seconds(value: Int) -> time.Duration
fn time.sleep(duration: time.Duration) -> Result<Unit, Cancelled>
fn check_cancel() -> Result<Unit, Cancelled>
```

`time.Duration` is opaque and non-negative. Negative constructors panic with
`K1501`; overflow follows the integer-overflow contract. A duration can be used
as a relative structured-concurrency deadline:

```keel
use std.time

fn work() -> Result<Unit, Cancelled> {
    time.sleep(time.milliseconds(10))?
    check_cancel()
}
```

`sleep` and `check_cancel` observe the ambient `scope`. Outside a scope no
cancellation is pending.

## `std.json`

```text
fn json.parse<T>(input: String) -> Result<T, json.Error>
fn json.parse<T>(input: String, mode: .tolerant) -> Result<T, json.Error>
fn json.write<T>(value: T) -> String
```

Parsing is strict by default: unknown and duplicate fields are errors. Tolerant
mode ignores unknown fields but retains the remaining validation. Writing is
deterministic and uses declaration order for struct fields.

Conformance-backed representations include primitives, `Option<T>`, `List<T>`,
concrete structs/enums, and the `Uuid`, `Timestamp`, and `Email` boundary
scalars. Unsupported targets are rejected statically with `K1503`.

Struct fields omitted from JSON must be `Option<T>` or have behavior explicitly
defined by the spec. Enum JSON uses the adjacent-tagged form described in
chapter 15.

`json.Error` values classify syntax errors, duplicate/unknown fields, missing
fields, and type mismatches. Handle them with `catch`, `match`, or `?`; do not
parse their rendered message.

## `std.http`

```text
fn http.serve(port: Int, routes: http.Router) -> Result<Unit, http.Error>

fn req.path_param<T>(name: String) -> Result<T, String>
fn req.query_param<T>(name: String) -> Option<T>

fn http.ok(body: String) -> http.Response
fn http.created(body: String) -> http.Response
fn http.no_content() -> http.Response
fn http.bad_request(err: Error) -> http.Response
fn http.not_found() -> http.Response
fn http.conflict(body: String) -> http.Response
fn http.internal_error(err: Error) -> http.Response
```

`http.Request` exposes `body`, `method`, and `path`. `http.Response` exposes
`status` and `body`. Both types are constructed by the runtime, not by ordinary
struct literals.

Routes map an uppercase method/path pattern to a handler or capturing closure:

```keel
use std.http

fn get_user(req: http.Request) -> http.Response {
    let id = req.path_param<Int>("id")
        catch err => return http.bad_request(err)
    http.ok("user {id}")
}

fn main() -> Result<Unit, http.Error> {
    http.serve(8080, http.Router{
        "GET /users/{id}": get_user,
    })
}
```

Handlers must have type `fn(http.Request) -> http.Response`; invalid handlers
produce `K1504`. Runtime ports outside 1–65535 panic with `K1505`.

Chapter 15 also specifies raw `req.query(name)` and `req.header(name)` methods,
but the current Go backend has no emission path for them and no conformance case
locks them. Use the conformance-backed typed parameter methods above.

## `std.log`

```text
fn log.info(message: String) -> Unit
fn log.warn(message: String) -> Unit
fn log.error(message: String) -> Unit
```

Each function writes one line to stdout with `[info]`, `[warn]`, or `[error]`.
There is no stable filtering, structured-field, or configurable-output surface.
Although the current compiler accepts named fields on log calls, chapter 15
marks that syntax aspirational and no conformance case defines its output.

## `std.sql`

```text
fn sql.connect(connection_string: String) -> Result<sql.Pool, sql.Error>

fn pool.query(statement: String, arguments...) -> Result<sql.QueryResult, sql.Error>
fn pool.query_one(statement: String, arguments...) -> Result<sql.Row, sql.Error>
fn pool.exec(statement: String, arguments...) -> Result<Int, sql.Error>
fn pool.migrate(statements: String) -> Result<Unit, sql.Error>

fn result.map(mapper) -> sql.RowMapper<T>
fn mapper.collect() -> Result<List<T>, sql.Error>
fn Struct.from_row(row: sql.Row) -> Result<Struct, sql.Error>
```

SQLite is the only bundled driver. Use `:memory:` or a SQLite data-source name.
The runtime recognizes PostgreSQL/MySQL URL prefixes, but those Go drivers are
not linked, so those connection strings are not currently usable.

Parameters use `$1`, `$2`, and so on:

```keel
use std.sql

struct User {
    id: Int
    name: String
}

fn find(db: sql.Pool, id: Int) -> Result<User, sql.Error> {
    let row = db.query_one(
        "select id, name from users where id = $1",
        id,
    )?
    User.from_row(row)
}
```

`query_one` returns `sql.NoRows` for zero rows and `sql.QueryFailed` for more
than one. `sql.UniqueViolation` is available as a qualified catch pattern.
`migrate` splits statements on semicolons and is not a versioned migration
system.

The typechecker recognizes `result.next()`, but the Go backend does not emit it
and no conformance case exercises it. Use `map(...).collect()` or
`Struct.from_row(query_one(...))`.

SQL builds currently resolve `modernc.org/sqlite` with `go mod tidy`, which may
access the network on the first build.

## `std.config`

```text
fn config.load<T>() -> Result<T, config.Error>
fn secret.unwrap() -> String
```

`T` must be a named struct whose fields are loadable. Field names map from
`snake_case` to `UPPER_SNAKE_CASE`. Supported field shapes are `String`, `Int`,
`Float`, `Bool`, `Option<T>`, and `Secret`; declaration defaults are used when
the environment variable is absent.

```keel
use std.config

struct AppConfig {
    database_url: Secret
    port: Int = 8080
}

fn load() -> Result<AppConfig, config.Error> {
    config.load<AppConfig>()
}
```

`Secret` is distinct from `String`; call `unwrap()` only at the boundary that
needs the value. Unsupported target structs produce `K1507`.

## Boundary scalar types

`Uuid`, `Timestamp`, and `Email` are available without an import. They are
opaque, comparable with their own type, renderable in interpolation, and
JSON-representable.

```text
fn Uuid.new() -> Uuid
fn Timestamp.now() -> Timestamp
```

- `Uuid` uses canonical lower-case RFC-variant version-4 text.
- `Timestamp` accepts RFC 3339 input and renders normalized UTC text.
- `Email` accepts the chapter-15 ASCII subset, preserves local-part case, and
  lowercases the domain.

These types validate syntax and canonical form; `Email` does not prove mailbox
ownership.
