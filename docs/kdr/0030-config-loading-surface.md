# KDR-0030: `std.config` — configuration loading surface

- **Status:** proposed
- **Date:** 2026-06-19
- **Scope:** stdlib

## Decision

`std.config` is a compiler-known, typed configuration loading module. Its surface for M6:

**Loading function**

```text
fn config.load<T>() -> Result<T, config.Error>
```

— `config.load<T>` is a compiler-known call (not a user-callable function). The type argument `T` must be a named struct type declared at module scope whose fields are all loadable. If `T` is not a named struct or cannot be configured from env vars, the compiler emits `K1507`.

— The function reads environment variables keyed by uppercasing the struct field name. For example:

  - `database_url` → `DATABASE_URL`
  - `port` → `PORT`
  - `cache_ttl_seconds` → `CACHE_TTL_SECONDS`

— For struct field `f`, the corresponding environment variable is looked up via `os.Getenv(f.uppercase())`. If the env var is not set:

  - If `f` has a default value in the struct declaration (`f: Type = default_value`), the default is used.
  - If `f` is `Option`-typed, it becomes `Some(value)` if the env var is set, `None` if not.
  - If `f` has no default and is not `Option`, the load returns `Err(MissingEnvVar(...))`.

— The env var value is parsed into the target type according to the mapping rules (see §KDR-0030 Numeric and string mapping below). If the env var value cannot be parsed, the load returns `Err(ParseError(...))`.

**Secret value type**

```keel
struct Secret {
    value: String,
}

fn secret.unwrap() -> String
```

— `Secret` is a compiler-known struct in the `config` namespace. It holds a single `String` field.

— Used to mark config fields that carry sensitive data (database URLs, API keys). When a `Secret` field encounters a missing env var, the error is `MissingSecret(...)` rather than `MissingEnvVar(...)` — same kind of missing-field error, more descriptive for logging and audit.

— `Secret` is distinct from `String` at the type level: `json.write(secret)` writes `<secret>` rather than the actual value. This is a safety belt: accidental serialization of secrets is caught by the type system (at the cost of boilerplate `secret.unwrap()` when you actually need the string).

**config.Error type**

```text
enum config.Error {
    MissingEnvVar(field_name: String),     // required field's env var is not set
    MissingSecret(field_name: String),     // Secret field's env var is not set
    ParseError(field_name: String, type: String, message: String), // env var cannot be parsed
    InvalidStructType(type_name: String),  // T is not a loadable struct
}
```

**Parse rules**

| Target type | Env var content | Result |
|---|---|---|
| `String` | any non-empty string | string value |
| `Int` | digit sequence, optional leading `-` | parsed as `Int64` |
| `Float` | float notation (e.g. `"3.14"`) | parsed as `Float64` |
| `Bool` | `"true"`, `"1"`, `"yes"`, `"on"` | `true` |
| `Bool` | `"false"`, `"0"`, `"no"`, `"off"` | `false` |
| `Bool` | anything else | parse error |
| `Secret` | any non-empty string | wraps value |
| `Option<T>` | non-empty string | `Some(parsed)` |
| `Option<T>` | empty string (`""`) | `None` |
| `Option<T>` | not set | `None` |

An empty string (`""`) is not treated as "not set" — it is treated as a value. For `String` and `Secret`, an empty string is a valid value. For `Int` and `Float`, an empty string is a parse error. For `Option<T>`, an empty string is `None`.

**Rejected alternatives**

- **File-based configuration (YAML, TOML, JSON).** Rejected for M6: adds parser dependencies, format choice, and file I/O. The 12-factor app pattern (environment variables only) is sufficient for M6. File-based loading is post-M6, enabled by `std.fs` and format-specific parsers.

- **Struct annotations.** `#[config:env("DB_URL")]` to remap field names to different env var names. Rejected: KDR-0004 prohibits annotations. The `snake_case → UPPER_SNAKE_CASE` mapping is the only transformation. For cases where this doesn't work (e.g. Google Cloud's `GOOGLE_APPLICATION_CREDENTIALS` → `credentialsFile`), the user must work around it in their struct design or post-M6 when more sophisticated mapping is added.

- **`.env` file support.** Rejected for M6: file I/O requires `std.fs`. Environment variables are loaded from the process environment, not from files. If a team needs `.env` loading (local development), they should use a `.envrc` tool (direnv, etc.) or a separate script.

- **`Secret<T>` generic wrapper.** Would allow wrapping `Int`, `Float`, etc. in a secret. Rejected: M6 only needs `Secret` for string-like sensitive data (database URLs, API keys). Numeric secrets are extremely rare and would complicate the type system (requiring generic struct support, which is constrained to interfaces in M5 but not yet usable for `Secret` with non-interface types).

- **Builder pattern.** `ConfigBuilder::new().bind(...).load()` — deferred. The `config.load<T>()` function is the only surface. If teams need more control (per-key overrides, defaults that change at runtime), a builder API is added later.

- **Nested config structs.** `struct Outer { inner: Inner }` — a field that is itself a struct with `config.load` behavior. Rejected for M6: requires recursive type metadata support. Flat config is the default and what the users-service needs.

- **Inline anonymous structs in `config.load<T>()`.** `config.load<struct { port: Int }>()` — passing an anonymous struct type as the type argument. Rejected: anonymous struct types are not yet a language feature. Config structs must be declared at module scope with a name (e.g. `struct AppConfig { ... }`). This is consistent with how all other struct types work in Keel.

## Context

The users-service example (`examples/users-service/main.keel`) requires loading the database connection string and port from configuration. The design choices come from:

**The 12-factor app** config principle: configuration is in the environment. No files, no versioned config, no environment-specific config files. This is the simplest possible loading surface and covers the vast majority of production deployments (containerized workloads, PaaS, Kubernetes).

**Go's `os.Getenv`** — the simplest possible API surface. A single function that takes a string key and returns a string value. `std.config` wraps this with type safety and the `Secret` marker.

**Django's `os.environ` config pattern** — named struct fields map to env vars via uppercasing. This is a convention used by many frameworks; making it explicit in the stdlib means it's standardized and tested rather than scattered across services.

**Rust's `dotenv` / `config` crates** — `dotenv` loads `.env` files (rejected for M6), `config` uses layered sources (env vars, files, defaults). `std.config` for M6 is the "env vars only" slice.

## Consequences

- The `snake_case → UPPER_SNAKE_CASE` mapping is not configurable — the only way to load a config with a non-standard env var name is to use a struct field with the exact env var name's casing, or to implement a custom loader post-M6.
- `Secret` is a concrete (non-generic) struct — it only wraps `String`. This limits what can be sensitive (e.g. no `Secret<Int>` for secret API keys that are numeric), but covers the common use case and avoids generic struct complications.
- Empty env vars behave differently by type: `String` gets `""`, `Option<T>` gets `None`, `Int`/`Float` get a parse error. This is deliberate — an empty string is a valid string value, but `""` is not a valid integer.
- `config.load<T>()` is a compiler-known call, not a user-callable function. The compiler generates the Go code to read env vars and parse them, similar to how `json.parse<T>()` works.
- The `T` type argument must be a named struct declared at module scope (e.g. `struct AppConfig { ... }`). Inline anonymous structs are not supported in M6.
- The function name approach is consistent with `http.serve(routes)` — the compiler resolves `load<T>` and generates the adapter code. The `T` type parameter is the struct to populate.

## Reopening clause

1. Corpus evidence that `snake_case → UPPER_SNAKE_CASE` is insufficient for ≥3 distinct Keel services (and that the workaround of using differently named struct fields is not adequate), AND that remapped env var names via a non-annotation mechanism can be implemented without violating KDR-0004.
2. Evidence that `Secret` for non-String sensitive data (numeric secrets, byte arrays) is a real need and cannot be worked around with a `String` wrapper + `.unwrap()` + parsing.
