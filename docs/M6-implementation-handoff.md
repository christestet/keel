# M6 Implementation Handoff

Generated after KDR/spec/conformance alignment. All design decisions are
settled. What remains is compiler implementation — each feature is its own
PR following the spec → tests → implementation sequence (hard rule 1).

---

## 1. HTTP Router + Closures + Params

**The ask:** Replace the old single-handler `http.serve(port, handler)` with
`http.serve(port, routes: http.Router)` where Router is a struct literal
with closure handler values.

### Design docs

| Document | File | What it says |
|---|---|---|
| KDR-0031 | `docs/kdr/0031-http-router-and-params.md` | Router type, closure handlers, typed path_param/query_param |
| KDR-0028 | `docs/kdr/0028-http-server-surface.md` | OLD (superseded by 0031) — single-handler model |
| Spec §15.17 | `docs/spec/15-stdlib-core.md` lines 311–348 | Router struct, http.serve signature |
| Spec §15.18 | `docs/spec/15-stdlib-core.md` lines 351–385 | Request fields, path_param<T>, query_param<T> |
| Spec §15.22 | `docs/spec/15-stdlib-core.md` lines 428–437 | K1504 (invalid handler), K1505 (invalid port) |

### Conformance tests

| Test | File | What it tests |
|---|---|---|
| 744 | `tests/conformance/744-http-serve-compiles/` | `http.serve` with Router compiles (build) |
| 745 | `tests/conformance/745-http-invalid-handler/` | Non-handler route value rejected with K1504 (reject) |
| 767 | `tests/conformance/767-http-router-closure-compiles/` | Router with closure capturing db compiles (build) |
| 768 | `tests/conformance/768-http-path-param/` | `path_param<T>` extraction compiles (build) |
| 769 | `tests/conformance/769-http-query-param/` | `query_param<T>` extraction compiles (build) |

### Existing compiler code to modify

| What | File | Lines | What to change |
|---|---|---|---|
| Resolver: http.serve validation | `compiler/keelc-resolve/src/lib.rs` | ~1190–1208 | Change from single-handler to Router validation |
| Backend: http.serve codegen | `compiler/keelc-backend-go/src/lib.rs` | ~1372 | Change from `keelHTTPServe(port, handler)` to Router-based |
| Backend: runtime struct | `compiler/keelc-backend-go/src/runtime.rs` | ~274–275 | Update `keelHTTPServe` signature, add routing runtime |
| Backend: serve detection | `compiler/keelc-backend-go/src/analysis.rs` | ~191–201 | Update `module_uses_http_serve` if needed |
| Error code K1505 | `compiler/keelc-diag/src/registry.rs` | registered | Wire up port validation in resolver |

### API signatures (from KDR-0031)

```text
// Router construction
http.Router{
    "GET    /users":      handle_list,           // bare function name
    "POST   /users":      fn(req) => create(req), // closure capturing vars
}

// http.serve
fn http.serve(port: Int, routes: http.Router) -> Result<Unit, http.Error>

// Request methods
fn req.query(name: String) -> Option<String>
fn req.header(name: String) -> Option<String>
fn req.path_param<T>(name: String) -> Result<T, String>  // parse + error
fn req.query_param<T>(name: String) -> Option<T>          // parse + default via ??
```

### Example usage (from users-service/main.keel)

```keel
fn routes(db: sql.Pool) -> http.Router {
    http.Router{
        "POST   /users":      fn(req) => handle_create(db, req),
        "GET    /users/{id}": fn(req) => handle_get(db, req),
    }
}

fn handle_get(db: sql.Pool, req: http.Request) -> http.Response {
    let id = req.path_param<Uuid>("id")
        catch err => return http.bad_request(err)
    // ...
}

fn handle_list(db: sql.Pool, req: http.Request) -> http.Response {
    let limit = req.query_param<Int>("limit") ?? 50
    // ...
}
```

---

## 2. SQL Database Access

**The ask:** Implement `std.sql` — connection pool, query execution,
migrations, and row-to-struct mapping.

### Design docs

| Document | File | What it says |
|---|---|---|
| KDR-0029 | `docs/kdr/0029-sql-database-access.md` | Pool model, pool.migrate, FromRow, all error variants |
| Spec §15.28 | `docs/spec/15-stdlib-core.md` lines 507–658 | Full SQL surface: connect, query, migrate, row mapping, errors |

### Conformance tests

| Test | File | What it tests |
|---|---|---|
| 770 | `tests/conformance/770-sql-connect/` | `sql.connect` compiles (build) |
| 771 | `tests/conformance/771-sql-migrate/` | `pool.migrate` compiles (build) |
| 772 | `tests/conformance/772-sql-query/` | `pool.query` compiles (build) |
| 773 | `tests/conformance/773-sql-query-one/` | `pool.query_one` compiles (build) |
| 774 | `tests/conformance/774-sql-exec/` | `pool.exec` compiles (build) |
| 775 | `tests/conformance/775-sql-row-mapping/` | `result.map(f).collect()` compiles (build) |

### Compiler code to create/modify

| What | File | Action |
|---|---|---|
| Error code K1506 | `compiler/keelc-diag/src/registry.rs` | Already registered — wire up in resolver |
| Resolver: sql.connect | `compiler/keelc-resolve/src/lib.rs` | Add `sql.connect(String) -> Result<sql.Pool, sql.Error>` |
| Resolver: pool methods | same | Add `pool.query`, `pool.query_one`, `pool.exec`, `pool.migrate` |
| Resolver: result.map | same | Add `QueryResult.map(f)` accepting bare function name |
| Resolver: mapper.collect | same | Add `RowMapper.collect()` |
| Backend: Go runtime | `compiler/keelc-backend-go/src/runtime.rs` | Add `database/sql` wrapper functions |
| Backend: codegen | `compiler/keelc-backend-go/src/lib.rs` | Generate calls to Go runtime |

### API signatures (from KDR-0029)

```text
fn sql.connect(connectionString: String) -> Result<sql.Pool, sql.Error>
fn pool.query(sqlStatement: String) -> Result<QueryResult, sql.Error>
fn pool.query_one(sqlStatement: String) -> Result<Row, sql.Error>
fn pool.exec(sqlStatement: String) -> Result<Int, sql.Error>
fn pool.migrate(migrationStatements: String) -> Result<Unit, sql.Error>
fn result.map(f: <fn(Row) -> T>) -> RowMapper<T>
fn mapper.collect() -> Result<List<T>, sql.Error>
```

### Error type

```keel
enum sql.Error {
    ConnectionFailed(message: String),
    QueryFailed(message: String),
    NoRows,
    UniqueViolation(message: String),
    MigrationFailed(message: String),
}
```

### Example usage (from users-service/main.keel)

```keel
let db = sql.connect(cfg.database_url.unwrap())?
db.migrate("create table if not exists users (id uuid primary key)")?
let row = db.query_one("select id, name from users where id = $1", id)?
let rows = db.query("select id, name from users")?
let users = rows.map(User.from_row).collect()?
```

---

## 3. Config Loading

**The ask:** Implement `std.config` — env var loading into typed structs,
`Secret` type for sensitive values.

### Design docs

| Document | File | What it says |
|---|---|---|
| KDR-0030 | `docs/kdr/0030-config-loading-surface.md` | config.load<T>, Secret, parse rules, error variants |
| Spec §15.31 | `docs/spec/15-stdlib-core.md` lines 700–802 | Full config surface: load, env mapping, parse rules, errors |

### Conformance tests

| Test | File | What it tests |
|---|---|---|
| 776 | `tests/conformance/776-config-load/` | `config.load<T>()` with named struct compiles (build) |
| 777 | `tests/conformance/777-config-secret/` | `Secret` with `unwrap()` compiles (build) |
| 778 | `tests/conformance/778-config-missing/` | `config.load<T>()` with required fields compiles (build) |

### Compiler code to create/modify

| What | File | Action |
|---|---|---|
| Error code K1507 | `compiler/keelc-diag/src/registry.rs` | Already registered — wire up in resolver |
| Resolver: config.load | `compiler/keelc-resolve/src/lib.rs` | Add `config.load<T>()` with struct validation |
| Resolver: Secret type | same | Add `Secret` as compiler-known struct |
| Resolver: secret.unwrap | same | Add `secret.unwrap() -> String` |
| Backend: Go runtime | `compiler/keelc-backend-go/src/runtime.rs` | Add `os.Getenv` + parsing functions |
| Backend: codegen | `compiler/keelc-backend-go/src/lib.rs` | Generate env var loading per struct field |

### API signatures (from KDR-0030)

```text
fn config.load<T>() -> Result<T, config.Error>   // T must be named struct
struct Secret { value: String }
fn secret.unwrap() -> String
```

### Error type

```keel
enum config.Error {
    MissingEnvVar(field_name: String),
    MissingSecret(field_name: String),
    ParseError(field_name: String, type: String, message: String),
    InvalidStructType(type_name: String),
}
```

### Env var mapping

| Field name | Env var |
|---|---|
| `database_url` | `DATABASE_URL` |
| `port` | `PORT` |
| `cache_ttl_seconds` | `CACHE_TTL_SECONDS` |

### Parse rules

| Type | Env var content | Result |
|---|---|---|
| `String` | any string (including empty) | string value |
| `Int` | digit sequence, optional `-` | parsed Int64 |
| `Float` | float notation | parsed Float64 |
| `Bool` | `true/1/yes/on` or `false/0/no/off` | boolean |
| `Secret` | any string | wraps value |
| `Option<T>` | non-empty = `Some(parsed)`, empty/absent = `None` | option |

### Example usage (from users-service/main.keel)

```keel
struct AppConfig {
    database_url: Secret
    port: Int = 8080
}

fn main() -> Result<Unit, Error> {
    let cfg = config.load<AppConfig>()?
    let db = sql.connect(cfg.database_url.unwrap())?
    // ...
}
```

---

## 4. Still aspirational (not in M6 scope)

These are in the users-service example but not yet specified:

| Feature | Where in example | Why deferred |
|---|---|---|
| `Uuid`, `Timestamp`, `Email` types | struct User fields | Core scalar types, needs own KDR |
| `log.info("msg", key: value)` named args | log.info call | Language feature, needs own KDR |
| `fn main() -> Result<Unit, Error>` union return | main signature | Partially specified in KDR-0005 |

---

## 5. File index (all files touched in this session)

### KDRs
- `docs/kdr/0029-sql-database-access.md` — pool.migrate, updated
- `docs/kdr/0030-config-loading-surface.md` — named struct, updated
- `docs/kdr/0031-http-router-and-params.md` — NEW, supersedes 0028
- `docs/kdr/INDEX.md` — 0028 superseded, 0031 added

### Spec
- `docs/spec/15-stdlib-core.md` — §15.17, §15.18, §15.22, §15.28, §15.31 updated

### Conformance tests
- `tests/conformance/744-http-serve-compiles/` — updated to Router
- `tests/conformance/745-http-invalid-handler/` — updated to Router
- `tests/conformance/767-http-router-closure-compiles/` — NEW
- `tests/conformance/768-http-path-param/` — NEW
- `tests/conformance/769-http-query-param/` — NEW
- `tests/conformance/770-sql-connect/` — NEW
- `tests/conformance/771-sql-migrate/` — NEW
- `tests/conformance/772-sql-query/` — NEW
- `tests/conformance/773-sql-query-one/` — NEW
- `tests/conformance/774-sql-exec/` — NEW
- `tests/conformance/775-sql-row-mapping/` — NEW
- `tests/conformance/776-config-load/` — NEW
- `tests/conformance/777-config-secret/` — NEW
- `tests/conformance/778-config-missing/` — NEW

### Example
- `examples/users-service/README.md` — aspirational list updated

### Compiler (files to modify for implementation)
- `compiler/keelc-resolve/src/lib.rs` — resolver changes for all 3 features
- `compiler/keelc-backend-go/src/lib.rs` — codegen for Router, SQL, Config
- `compiler/keelc-backend-go/src/runtime.rs` — Go runtime functions
- `compiler/keelc-backend-go/src/analysis.rs` — module usage detection
- `compiler/keelc-diag/src/registry.rs` — K1505, K1506, K1507 already registered

---

## 6. PR sequence

Per hard rule 1: spec → tests → implementation. Spec and tests are done.
Each feature is its own PR.

| PR | Feature | What it does |
|---|---|---|
| 1 | HTTP Router | Resolver + backend for Router struct, closures, path_param, query_param, K1505 |
| 2 | SQL | Resolver + backend for pool.query/migrate/exec, row mapping, K1506 |
| 3 | Config | Resolver + backend for config.load<T>, Secret, K1507 |

Run `cargo run -p conformance-runner -- --check` before declaring each PR done.
Paste the summary (`suite ok: N case(s)`) in the PR description.
