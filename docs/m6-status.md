# M6 status — stdlib slice + the demo service

Single live note for M6. Non-normative. The governing language definition is
[`docs/spec/keel-core.md`](spec/keel-core.md); the executable spec is
[`tests/conformance/`](../tests/conformance/); milestone scope and exit criteria
live in [`ROADMAP.md`](../ROADMAP.md). Decisions are made in
[`docs/kdr/`](kdr/) under the three-PR discipline in [`AGENTS.md`](../AGENTS.md)
(spec → tests → impl, each its own concern).

This note plans work; it does not authorize it. Every step below is still a
KDR → spec → conformance → compiler sequence.

## Status

- M6 stdlib slice complete: `std.time`, `std.json`, `std.http`, `std.sql`,
  `std.config`, `std.log` (per-module notes: [json](#stdjson) / [log](#stdlog)
  below).
- Conformance (M6): **194 passed, 0 failed, 3 skipped**. M1–M5 preflight sweep
  green — no earlier gate regressed.
- **M6 exit reached.** [`examples/users-service/main.keel`](../examples/users-service/main.keel)
  compiles and runs full CRUD on SQLite (pure-Go `modernc.org/sqlite`,
  KDR-0042). Behavior locked by cases `804-sql-params-roundtrip` and
  `806-sql-crud-update-list`.

## The exit gap — CLOSED

ROADMAP M6 **exit** is: [`examples/users-service/main.keel`](../examples/users-service/main.keel)
compiles, runs, and passes its test file. The example is aspirational by
design (see [`examples/CLAUDE.md`](../examples/CLAUDE.md) and the
[service README](../examples/users-service/README.md)) — Core grew to meet it.

The four remaining gaps and the SQLite execution path are all resolved:

- **`Option<T>.unwrap()`** — KDR-0039, case 797.
- **`json.write` → `String`** — KDR-0040, cases 725/733/779/783/786/788/789/790/792.
- **http error helpers accept `Error`** — KDR-0041, case 798.
- **SQLite execution** — KDR-0042: pure-Go driver, module-mode build, `$N`→`?N`
  placeholder rewrite, parameter binding, `collect()` flattening; cases 804/806.
- Supporting codegen: `?`/`catch` ANF hoist (799), discarded `?` (805),
  `go_type(http.Router)` (800), from_row gating (803), `List<T>` JSON codec (802).

Already built before this wave: union error returns (`UserError | sql.Error`),
`?` union widening, `catch` + exhaustiveness (506/509/510), all five stdlib
surfaces, `fn main() -> Result<Unit, E>`, primitive
`path_param`/`query_param`/`json`.

## The steps (each: KDR → spec → conformance → compiler)

**Step 0 — Multi-line string literals. ✅ DONE** (KDR-0035). The example's SQL
queries span multiple physical lines inside one `"..."`; the lexer previously
terminated a string at the first newline (`K0002`). Core now accepts literal
newlines inside string literals. Blocked every other step.

**Step 1 — Universal `Error` type. ✅ DONE** (KDR-0033; spec §5; cases
511/512; impl in `type_absorbs` + catch/match opacity, `K0504`). `Error`
absorbs any propagated error at `?`/return and is opaque (no destructure;
`catch`/`match` on it is `K0504`).

**Step 2 — Core scalars `Uuid`/`Timestamp`/`Email`. ✅ DONE** (KDR-0034;
spec §15.34; cases 779–793). `Uuid` and canonicalized `Email` use string-backed
Go values; `Timestamp` uses separate epoch-seconds and nanosecond fields across
the RFC 3339 year range. JSON/HTTP parsing, constructors, canonical
interpolation, offset normalization, named-value JSON writing, and the
three-scalar struct round-trip are implemented.

**Step 3 — Call-site named arguments + structured `log.info`.** General
`name: value` at call sites, plus the structured-log output format. DoD:
`log.info("m", k: v)` output case + a named arg satisfying a `limit: Int = 50`
default. (Today `log.info/warn/error` take a bare `String` only — see
[std.log](#stdlog).)

**Step 4 — Derived `Struct.from_row`. ✅ DONE** (case 794-derived-from-row).
Auto-derive `Type.from_row(row) -> Result<Type, sql.Error>` for column-mappable
structs, usable both directly and as a first-class value in `rows.map(...)`,
reusing the existing `Type.method` call path.

**Step 5 — `sql.UniqueViolation` classification. ✅ DONE** (KDR-0037; case
796-sql-classification-catch, build-mode). Qualified classification patterns
`sql.NoRows`/`sql.UniqueViolation` in catch/match arms, catch propagation of
unmatched opaque errors, and the typed `Err(err: sql.Error)` match pattern.
Bundled with the union-narrowing patterns (KDR-0038; case
795-union-narrowing-patterns): nested destructuring, typed bindings `x: T`, and
the `()` unit pattern. Match arm bindings now carry their payload type.

**Step 6 — Example test harness + SQLite validation.** Write the users-service
test file and a runner that builds + runs it on SQLite. Likely **zero dialect
code** — SQLite already accepts `$NNN` params, `RETURNING`, and arbitrary
column type names — but this step is where that assumption is verified. This is
the exit gate.

## Remaining gaps

`main.keel` now parses and typechecks all the way through. It surfaces four
remaining gaps, each a **separate concern** (own KDR → spec → tests → impl), not
pattern work:

1. **`json.write(v)` ergonomics.** Returns `Result<String, json.Error>`, but the
   example uses it bare (`http.ok(json.write(user))`). Either it returns
   `String` (serializing a statically JSON-representable value cannot fail), or
   the http helpers accept the `Result`. Decide in a KDR.
2. **http error helpers accept an error value.** `http.bad_request(err)` /
   `http.internal_error(err)` are typed `(String)`; the example passes a
   `json.Error` / `sql.Error`. Widen them to render an error (how it stringifies
   is the spec decision).
3. **`Option<T>.unwrap()`** — `input.email.unwrap()` (K0606); only `Secret.unwrap`
   exists today.
4. **Step 6** — the test harness + SQLite end-to-end run, the literal exit gate.

(1)–(3) are small but each is a real semantic decision; (4) is the gate.

## Dependency chain

```
1 Error type ─┐
2 scalars ────┼─► example typechecks ─► 4 from_row ─► example lowers ─┐
3 named args ─┘        (4 needs 2 + sql.Row)                          ├─► 6 harness+test ─► M6 EXIT
5 sql.UniqueViolation ───────────────────────────────────────────────┘
```

## Milestone boundary

This is all M6 (the exit criterion itself). It does **not** reach into M7
([`ROADMAP.md`](../ROADMAP.md) "The differentiators"). Scalars (Step 2) are
Core scalar *types*, not the M7 structured-generation work.

## Validation snapshot

```
KEEL_MILESTONE=M6 scripts/preflight.sh   # green: 185 passed, 0 failed, 3 skipped
for m in M1 M2 M3 M4 M5; do KEEL_MILESTONE=$m scripts/preflight.sh; done  # all green
```

Each step lands its own conformance cases and must keep every milestone green.
The final gate (Step 6) additionally runs the example end-to-end on SQLite.

---

# Per-module implementation notes

## std.json

Contract: [`spec §15.7–15.16`](spec/15-stdlib-core.md) and
[`KDR-0027`](kdr/0027-json-boundary-mapping.md). **Done — cases 724–735 pass at
M6**, exercising struct round-trip, write order, missing/absent fields, strict
and tolerant unknown-field handling, duplicate-key rejection, integer overflow,
non-integer rejection, enum adjacent-tagged shape, trailing-input rejection, and
the compile-time `K1503` guard.

- **Parser** (`keelc-parse`): `parse_call_arg` recognises `mode: .tolerant` and
  encodes it as a sentinel string literal `"__keel_json_tolerant"` so KIR
  lowering can tell strict from tolerant without a new AST node. Type args on
  field-call expressions (`json.parse[T](…)`) gated to milestone ≥5 here.
- **Formatter** (`keelc-ast/pretty.rs`): detects the sentinel when the callee is
  `json.parse` and round-trips it as `mode: .tolerant`, so `keel fmt` is stable.
- **Typechecker** (`keelc-resolve`): `json.parse[T]` / `json.write(value)` call
  `ctx.is_json_representable`; non-representable types emit `K1503` at the
  type-arg or value span.
- **KIR lowering** (`keelc-kir/lower.rs`): the sentinel travels into KIR call
  args unchanged; the backend inspects it there.
- **Go backend**: statically-derived codecs `keelJSONParse_T` /
  `keelJSONDecode_T` / `keelJSONEncode_T`. Struct decoders check duplicate
  fields (in the token stream), missing required fields, optional defaulting to
  `None`, and `tolerant`-gated unknown fields. Enum decoders enforce
  `{"variant":"…","fields":{…}}`. Encoder writes fields in declaration order
  (determinism, §15.12). `keelJSONParseRaw` rejects trailing non-whitespace.
  Integer decoding rejects `.`/`e`/`E` then `strconv.ParseInt`.
- **Types** (`keelc-types/infer.rs`): `is_json_representable` implements the
  recursive check from §15.8 (`Unit`, function values, `TypeParam`, `Unknown` →
  false).

Prior-art deltas: Go `encoding/json` v1 accepts duplicate keys and decodes
numbers via `float64`; KDR-0027 rejects both — the duplicate-key check runs on
the token stream and integers go direct to `ParseInt`. Serde's attribute system
was rejected (KDR-0004, KDR-0027): no user-visible codec hooks, so the path can
be replaced wholesale when the native backend arrives (conformance is the proof).

**Not done (still M6):** `Map<String, T>` codec (representability passes, backend
hits the unsupported branch — blocked on a conformance case first, hard rule 8);
`json.Error` as a nameable Keel enum (variants are `KeelEnum` tags, catchable by
tag string but not type-annotatable); `keel gen` schema metadata (outside
KDR-0027 scope).

## std.log

Contract: [`spec §15.25–15.27`](spec/15-stdlib-core.md). **Done — cases 746–748
pass at M6.**

```keel
fn log.info(message: String) -> Unit
fn log.warn(message: String) -> Unit
fn log.error(message: String) -> Unit
```

Each writes to stdout with a `[level]` prefix. No error types, no structured
data, no filtering — YAGNI for M6.

| Crate | What changed |
|---|---|
| `keelc-types/src/infer.rs` | `infer_call` / `infer_method_call`: match `log.info\|warn\|error` → `Unit`. |
| `keelc-resolve/src/lib.rs` | `infer_call`/`infer_method_call`: match `log` + `check_call_args(&[String], ...)`; unknown methods emit `K0606`. |
| `keelc-backend-go/src/lib.rs` | `module_uses_log()`, `emit_log_call()`, `emit_log_runtime()` — Go funcs calling `fmt.Println("[info]", msg)` etc.; `uses_log` struct field. |

| Case | Checks |
|---|---|
| `746-log-info-output` | `log.info("hello")` → `[info] hello` |
| `747-log-warn-output` | `log.warn("careful")` → `[warn] careful` |
| `748-log-error-output` | `log.error("fail")` → `[error] fail` |

Structured key-value pairs need named args or Map literals (Step 3) — deferred.

---

# Stdlib API reference (http / sql / config)

Durable signatures for the three larger M6 surfaces. Normative source is the
spec section and KDR named per block; this is a quick index.

## std.http — Router + params (KDR-0031, supersedes 0028; spec §15.17–15.22)

```text
http.Router{
    "GET    /users":      handle_list,            // bare function name
    "POST   /users":      fn(req) => create(req),  // closure capturing vars
}
fn http.serve(port: Int, routes: http.Router) -> Result<Unit, http.Error>
fn req.query(name: String) -> Option<String>
fn req.header(name: String) -> Option<String>
fn req.path_param<T>(name: String) -> Result<T, String>
fn req.query_param<T>(name: String) -> Option<T>
```

Error codes: `K1504` (invalid handler), `K1505` (invalid port). Cases:
744, 745, 767, 768, 769.

## std.sql (KDR-0029; spec §15.28)

```text
fn sql.connect(connectionString: String) -> Result<sql.Pool, sql.Error>
fn pool.query(sqlStatement: String) -> Result<QueryResult, sql.Error>
fn pool.query_one(sqlStatement: String) -> Result<Row, sql.Error>
fn pool.exec(sqlStatement: String) -> Result<Int, sql.Error>
fn pool.migrate(migrationStatements: String) -> Result<Unit, sql.Error>
fn result.map(f: <fn(Row) -> T>) -> RowMapper<T>
fn mapper.collect() -> Result<List<T>, sql.Error>

enum sql.Error {
    ConnectionFailed(message: String), QueryFailed(message: String),
    NoRows, UniqueViolation(message: String), MigrationFailed(message: String),
}
```

Error code: `K1506`. Cases: 770–775.

## std.config (KDR-0030; spec §15.31)

```text
fn config.load<T>() -> Result<T, config.Error>   // T must be a named struct
struct Secret { value: String }
fn secret.unwrap() -> String

enum config.Error {
    MissingEnvVar(field_name: String), MissingSecret(field_name: String),
    ParseError(field_name: String, type: String, message: String),
    InvalidStructType(type_name: String),
}
```

Field-name → env-var: `database_url` → `DATABASE_URL`, etc. (uppercase snake).
Parse rules: `Int` (optional `-`), `Float` (float notation), `Bool`
(`true/1/yes/on` ↔ `false/0/no/off`), `Secret` (wraps any string), `Option<T>`
(non-empty = `Some`, empty/absent = `None`). Error code: `K1507`. Cases:
776–778.
