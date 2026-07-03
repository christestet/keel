# KDR-0029: `std.sql` — database access surface

- **Status:** accepted
- **Date:** 2026-06-19
- **Scope:** stdlib

## Decision

`std.sql` is a compiler-known, typed database module. Its surface for M6 is:

**Core types**

— `sql.Pool` — opaque compiler-known type representing a database connection pool. Created only through `sql.connect(connectionString: String) -> Result<sql.Pool, sql.Error>`. Cannot be constructed by user code.

— `QueryResult` — returned by `query(sqlStatement: String) -> Result<QueryResult, sql.Error>`. Opaque; provides `next() -> Option<Row>` for row iteration and `map(f: fn(Row) -> T) -> RowMapper[T]` for bulk mapping.

— `Row` — opaque compiler-known type. Provides no public fields in M6; row-to-struct mapping is done through the `map` method using compiler-provided column type metadata.

— `RowMapper[T]` — compiler-known wrapper returned by `QueryResult.map(f)`. Provides `collect() -> Result<List<T>, sql.Error>` to consume all rows into a list.

**Error type**

— `sql.Error` — opaque enum with variants:
  - `ConnectionFailed(message: String)` — returned by `sql.connect` when the connection string is invalid or the database is unreachable.
  - `QueryFailed(message: String)` — returned by `query`, `query_one`, or `exec` for general query errors (syntax error, constraint violation, etc.).
  - `NoRows` — returned by `query_one` when the query matches zero rows. Distinguishable in `catch` arms.
  - `UniqueViolation(message: String)` — returned when the query violates a unique constraint. Distinguishable in `catch` arms.
  - `MigrationFailed(message: String)` — returned when `migrate` encounters an error.

**API surface**

```text
fn sql.connect(connectionString: String) -> Result<sql.Pool, sql.Error>
```

— `sql.connect` returns a pool backed by the Go `database/sql` pool (default max open = 25, max idle = 2).

**Query execution and migrations**

```text
fn pool.query(sqlStatement: String) -> Result<QueryResult, sql.Error>
fn pool.query_one(sqlStatement: String) -> Result<Row, sql.Error>
fn pool.exec(sqlStatement: String) -> Result<Int, sql.Error>
fn pool.migrate(migrationStatements: String) -> Result<Unit, sql.Error>
```

— `query` returns a `QueryResult` carrying the full result set.

— `query_one` returns a single `Row`. Returns `Err(NoRows)` if zero rows match; returns `Err(QueryFailed)` if more than one row matches (the implementation fetches all rows and checks the count, not a database-side limit).

— `exec` returns the integer number of rows affected (0 for non-SELECT queries).

— `pool.migrate` runs all semicolon-separated SQL statements sequentially. Returns `Err` on the first failure; partial failures are not retried. The user-service M6 example uses `CREATE TABLE IF NOT EXISTS`, which is safe to run repeatedly. Migrations are intentionally not a compiler-time checked feature in M6.

**Row mapping — the `FromRow` pattern**

Row-to-struct mapping is done through a compiler-known `map` method on `QueryResult`, analogous to the `http.serve` handler model:

```text
// Signature convention — user provides a function matching this shape:
fn Row.from_user(row: Row) -> User   // or User.from_row, from_user, etc.
```

— `QueryResult.map(user_from_row)` — user passes a bare function name. The compiler verifies the function resolves to a function with a signature matching `fn(Row) -> TargetStruct` (where `TargetStruct` is inferred from the function's return type). If no such function exists or the signature does not match, the compiler emits `K1506`.

— `RowMapper.collect()` — consumes all remaining rows, calling the mapping function once per row. Returns `Result<List<T>, sql.Error>` where `T` is the struct type inferred from the mapping function's return type.

Row field types are matched positionally against the Go column type registry (e.g. `int64 → Int`, `text → String`, `timestamptz → Timestamp`). The compiler does not validate schema-to-struct alignment in M6; this is a known M7 improvement (see reopening clause).

**Rejected alternatives**

- **ORM with `struct` tags / annotations.** Rejected: KDR-0004 prohibits annotations and reflection. No `json:""`-style tags. The `FromRow` function name is the only row-mapping mechanism and is a direct function call, not a reflected dispatch.

- **`database/sql`-style `*Rows` handle leaking into user code.** Rejected: Go's `*sql.Rows` is a pointer with `Close()` obligation that is famously overlooked. `std.sql` holds the Go handle internally; `QueryResult` is a value type with `next()` for iteration. No explicit close needed.

- **Named parameters (`$1`, `$2` → `{name}`).** Keel uses the database-native `$1`, `$2` syntax. This is explicit about parameter position and maps directly to Go's `driver.NamedValue`. No macro-based parameter substitution.

- **Compile-time SQL verification.** Rejected for M6: requires SQL schema files, a migration hash index, and a compiler-time validation pass — all M7 capabilities (capabilities, `keel gen`, schema-driven generation).

- **Builder pattern for query construction.** Rejected: KDR-0004 and the absence of first-class function types in M6. Query strings are plain `String`. A builder API would require struct field accessors and method chains that Keel does not yet support.

- **`QueryResult<T>` generic type (typing the result at query call).** Would allow `result.rows<T>()` but requires knowing the target struct at query call time. The `map(f)` approach pushes the type to the mapping function, which is simpler: `result.map(from_user).collect()` clearly separates "get rows" from "decode rows."

- **Connection builder / options.** Rejected for M6: no config module is implemented yet, and pool options (max open, timeout) are encoded in the connection string. `sql.connect(connectionString)` is the only surface.

- **`sql.migrate` as a top-level function.** Rejected: `pool.migrate(statements)` is more ergonomic and consistent with the other pool methods (`pool.query`, `pool.query_one`, `pool.exec`). The pool already carries the connection; threading it as a separate argument adds verbosity with no benefit.

## Context

The users-service example (`examples/users-service/main.keel`) requires database access. The design choices come from observed costs in other languages:

**Go's `database/sql`** provides pool management and parameterized queries via `$1`, `$2`. It returns `*sql.Rows` — a pointer handle with explicit `Close()` — which is the single most common footgun in Go code (forgotten `defer rows.Close()`). The `map(f)` method pattern eliminates this: `QueryResult` owns the handle and closes it on iteration completion.

**Rust's `sqlx`** is the closest analog to Keel's approach: compile-time column-to-field mapping, a `FromRow` trait (here, a direct function instead of a trait), and zero runtime reflection. The cost: `sqlx` pulls in a large dependency tree and complex procedural macros. Keel gets the same compile-safety with a single compiler-known function check.

**Prisma / Drizzle** solve this with codegen from a schema file. M6 does not have `keel gen` integration for SQL; row mapping happens at runtime through the `FromRow` function, and compile-time verification is a future improvement.

**Django ORM / ActiveRecord** hide parameter types behind magic, producing queries with wrong types or N+1 patterns. Keel's explicit SQL text + positional parameters keeps queries visible and reviewable.

### Missing supporting types

The users-service example uses `Uuid`, `Timestamp`, and `Email` as types. These are not yet part of Core and do not land in M6. The `std.sql` surface does not define them; the users-service example cannot compile until they are introduced (likely as core scalar types or as a separate `std.primitive` module). This is tracked as an M6 blocking dependency.

## Consequences

- Row mapping is limited to one function per struct, preventing ambiguity but requiring boilerplate `from_row` functions. This is the "one way to do things" principle: every struct that maps from SQL has exactly one `fn ...from_row(row) -> Struct` function.
- Schema validation (column-to-field matching) is not M6. The compiler verifies the `FromRow` signature but not whether the struct fields align with the query result columns. M7 adds this via `keel gen` + schema files.
- Pool options (max connections, timeout, idle timeout) are controlled via the connection string. A pool configuration API is post-M6.
- The `pool.migrate()` method is a simple statement runner, not a migration tool with versioning. Production migrations use an external tool (e.g. `golang-migrate`) as a pre-deployment step. M6 migrations are for convenience in demos and tests.
- `sql.Error` variants are values, not exceptions. They travel through `Result` and are caught in `catch` arms using pattern matching on the enum, consistent with KDR-0005.
- `QueryResult.map()` accepts a function name, not a closure. This is the same constraint as `http.serve(handler)` and is dictated by the absence of function types in M6.

## Reopening clause

1. Corpus evidence that schema-to-struct alignment checking is a frequent source of bugs and cannot be adequately covered by testing, AND that a `keel gen`-driven schema file pipeline (introduced in M7+) can mechanically verify the mapping without user-level macros or reflection.
2. Evidence that `map(f)` with function-name-only semantics blocks a common real pattern, AND that closures (when added as a language feature) measurably reduce boilerplate for row mapping.
