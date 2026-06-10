# Keel Implementation Roadmap

Milestones are in strict dependency order. Each milestone has a binary exit
criterion. Do not start milestone N+1 work while N's exit criterion fails.

## M0 — Freeze Keel Core and its conformance suite  *(no compiler code)*

Define the minimal language subset in `docs/spec/keel-core.md` and express it as
executable conformance tests in `tests/conformance/`. The subset: functions,
`let`/`mut`, primitive types, `String`, `struct`, `enum` (with payloads),
`match` (exhaustive), `Option`/`Result`, `?`, `catch`, blocks/expressions,
`List<T>`/`Map<K,V>` (built-in, not user generics yet), string interpolation,
modules/`use`, `print` for test output. Explicitly deferred: user-defined
generics, interfaces, `scope`/`spawn`, `arena`, capabilities, FFI, stdlib.

**Exit:** ≥ 60 conformance cases covering both accept (run + expected stdout)
and reject (expected compile error code) behavior, reviewed and frozen.

## M1 — Frontend: lexer, parser, AST, diagnostics

Implementation language: **Rust** (see `compiler/ARCHITECTURE.md`, KDR-101).
Hand-written recursive-descent parser (good errors beat generated parsers).
Diagnostics carry stable error codes (`K####`) from day one — conformance
reject-tests match on codes, not message text.

**Exit:** every M0 case lexes+parses or fails with the right `K####` syntax code.

## M2 — Semantic analysis: resolver + typechecker

Name resolution, type checking (explicit signatures, local inference only),
exhaustiveness checking for `match`, no-implicit-zero struct construction,
`Option`/`Result` semantics, `?`/`catch` typing with union error types.

**Exit:** all M0 reject-cases produce their exact error codes; accept-cases typecheck.

## M3 — First backend: compile to Go  *(KDR-102)*

Lower the typed AST to a Keel IR, emit Go source, drive `go build` internally.
This buys a production-grade concurrent GC, scheduler, cross-compilation and
static binaries for free, making Keel programs *runnable* years earlier. The Go
backend is scaffolding, not destiny: a native backend replaces it later, and the
conformance suite is what guarantees identical behavior when it does.

**Exit:** `keelc run` passes 100% of M0 accept-cases. `examples/hello.keel` works.

## M4 — Toolchain skeleton: `keel` CLI, `fmt`, `test`

Single binary UX: `keel build|run|fmt|test`. `keel fmt` is canonical from the
first release — formatting freezes *now*, while the corpus is small.
`keel test` discovers `test "name" { }` blocks; `assert` with structural diffs.

**Exit:** `keel test` runs a Keel-language test file; `keel fmt` is idempotent on the whole repo.

## M5 — Language completion wave 1

Interfaces (≤5 methods, compiler-enforced), user generics (interface-constrained
only), `scope`/`spawn` structured concurrency on the Go runtime, resource scoping.

## M6 — Stdlib slice + the demo service

`std.http`, `std.json`, `std.sql` (SQLite first, then Postgres), `std.log`,
`std.config`. **Exit:** `examples/users-service/main.keel` from the design
discussion compiles, runs, and passes its test file.

## M7 — The differentiators

Package manifests + capability enforcement, `keel audit`, `arena` blocks,
`keel gen` for protobuf/OpenAPI, hermetic builds, edition machinery in the
compiler (must exist before 1.0 even though edition 2 is years away).

## Performance contract (applies from M1 onward)

CI tracks compile time on a growing reference corpus. Regressions > 5% block merge
(vision.md §7). Incrementality is architecture, not a later feature: the compiler
is query-based (salsa-style) from M1.
