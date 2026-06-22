# Idiomatic Keel

This guide is non-normative. It explains how the accepted design and current
conformance cases fit together in ordinary service code. The specification wins
on semantics.

## Make invalid states explicit

Use structs and enums to describe the states the program actually permits.
Use `Option<T>` only for genuine absence; do not encode absence with sentinel
strings or numbers.

```keel
enum AccountState {
    Active,
    Suspended(reason: String),
    Deleted,
}
```

Construct every required struct field. Defaults are appropriate only when the
same value is correct for every omitted call site.

## Match variants by name

Prefer named arms over `_` when matching an enum declared in the same module:

```keel
match state {
    Active            => serve(),
    Suspended(reason) => explain(reason),
    Deleted           => gone(),
}
```

This makes a newly added variant break the places that need a decision. A
wildcard is useful at open boundaries, not as a shortcut around exhaustiveness.

## Keep recoverable failure in `Result`

Return a specific error enum or union from domain functions:

```keel
enum UserError {
    NotFound(id: Uuid),
}

fn load_user(id: Uuid) -> Result<User, UserError | sql.Error> {
    // ...
}
```

Use `?` when the caller has the same policy and `catch` where the program can
recover or translate the error. Reserve the opaque `Error` type for boundaries
that only propagate/log, such as `main`; using it deep in the program removes
the ability to branch by error type.

`panic` means a program invariant is broken. Network, database, configuration,
and user-input failures are not panics.

## Treat `unwrap()` as an assertion

`Option<T>.unwrap()` is appropriate only when earlier logic proves the value is
present and absence would be a bug. Otherwise match, use `?`, or preserve the
option.

```keel
match user.email {
    Some(email) => send(email),
    None        => skip(),
}
```

## Parse at boundaries

External strings become honest types at one visible parse point:

- `json.parse<T>` for request bodies;
- `path_param<T>`/`query_param<T>` for HTTP parameters;
- `config.load<T>` for environment configuration;
- `Struct.from_row` for SQL rows.

Keep JSON strict by default. Use tolerant mode only when the producer is allowed
to add fields independently and that compatibility policy is intentional.

Use `Uuid`, `Timestamp`, and `Email` at boundaries instead of carrying validated
strings through the program.

## Parameterize SQL

Pass values separately from statement text:

```keel
db.query_one(
    "select id, name from users where id = $1",
    id,
)?
```

Do not build SQL by string interpolation. `pool.migrate()` is suitable for
examples/tests, not as a production migration history system.

## Keep interfaces small and concrete

An interface should express the few operations a caller needs. The language
enforces a maximum of five methods; most interfaces should need fewer.

Use an explicit `impl` and accept an interface where dynamic dispatch is the
point. Use a constrained generic when the function only needs the bound and
should preserve the concrete input type through checking.

Do not create interfaces merely to wrap one implementation or predict a future
extension. A plain struct/function is easier to read and change.

## Keep concurrency inside `scope`

Spawn independent work together and consume task values at the scope's join
barrier:

```keel
let result = scope {
    let profile = spawn load_profile(id)
    let settings = spawn load_settings(id)
    combine(profile.value, settings.value)
}
```

Do not emulate detached background work. Propagate `Cancelled`, use deadlines at
request/service boundaries, and call `check_cancel()` in long compute loops.

## Use arenas only for measured hot paths

General allocation belongs to the GC. `arena` is for request-scoped graphs or
other measured allocation pressure, not a default block around every function.

The current Go backend does not allocate a real region, so adding arenas today
does not improve runtime performance. Use them only where their lexical lifetime
communicates intent that the M11 backend can eventually enforce completely.

## Make authority visible

Use an explicit `keel.toml` for any service or library. Declare only the
capabilities the package and dependencies need, and review `keel audit` when the
dependency graph changes.

Do not rely on implicit single-file packages for security-sensitive code; M7
skips capability enforcement for them.

## Keep secrets narrow

Load secrets as `Secret`, call `unwrap()` at the API that requires the raw
string, and do not retain/log the result. The type is a guardrail, not a vault or
automatic redaction system.

## Let the formatter decide

Run `keel fmt` and commit its output. There are no formatting options and no
project style variants. Avoid hand-aligning code in ways the formatter removes.

## Prefer the boring shape

Keel intentionally excludes macros, reflection, inheritance, operator
overloading, exceptions, and `async`/`await`. Solve service problems with plain
data, explicit functions, small interfaces, exhaustive matches, and visible
boundaries. If a solution needs hidden control flow or generated language
surface, reconsider the design before proposing a feature.
