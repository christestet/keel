# keelc — Compiler Architecture

## Implementation language: Rust ([KDR-0101](../docs/kdr/0101-compiler-in-rust.md))

Chosen for: memory-safe compiler internals, excellent error-handling ergonomics
for diagnostics, `salsa` for query-based incremental compilation, mature parser
and IR tooling, and single-binary distribution. Self-hosting Keel-in-Keel is a
post-1.0 aspiration, not a plan.

## Pipeline

```
source --> lexer --> parser --> AST --> resolver --> typechecker --> KIR --> backend
            |          |                    |             |                    |
            +----------+-----  diagnostics (stable K#### codes)  -------------+
```

- **Query-based core ([salsa](https://github.com/salsa-rs/salsa)-style) is the
  target architecture.** Every stage is designed as a memoized query keyed on
  inputs. This is how the [vision.md §7](../docs/vision.md#7-compile-time-as-a-contract)
  budget (incremental < 1s, `keel check` < 300ms) stays achievable. The salsa
  integration is not yet implemented; the frontend currently drives stages
  directly. Retrofitting incrementality is the single most expensive mistake a
  compiler project makes (see: rustc).
- **Lexer/parser:** hand-written recursive descent. Parser must recover from
  errors (parse the whole file, report many diagnostics, never crash).
- **AST → typed HIR:** name resolution, then type checking. Explicit function
  signatures; inference is local (`let`) only. Exhaustiveness checking is part
  of typechecking, not a lint.
- **KIR (Keel IR):** small, explicitly-typed, desugared (e.g. `?` and `catch`
  become explicit match-and-return). All backends consume KIR only.
- **Backends:** `backend-go` first ([KDR-0102](../docs/kdr/0102-go-backend-first.md)) — emits readable Go, drives the Go
  toolchain for codegen, GC, scheduler, cross-compile, static linking.
  `backend-native` (LLVM or cranelift) replaces it later; the conformance suite
  is the equivalence proof.

## Diagnostics are a public API

Every error has a stable code `K####`, a primary span, and a "what to do" line.
Conformance reject-tests assert on codes. Changing a code is a breaking change.
Error message *text* may improve freely.

## Crate layout (suggested)

```
compiler/
  keelc-span        source maps, spans, file ids
  keelc-diag        diagnostic types, K#### registry (registry is a checked-in file)
  keelc-lex         lexer
  keelc-parse       parser -> AST
  keelc-ast         AST definitions (+ pretty printer = the formatter's core)
  keelc-resolve     name resolution
  keelc-types       type definitions (TypeInfo, merge, collect) + typechecker -> typed HIR
  keelc-kir         IR + lowering
  keelc-backend-go  Go emission
  keelc-driver      query database, CLI entry; builds both the user-facing
                    `keel` binary and the `keelc` binary used by the
                    conformance runner
  conformance-runner  test harness for tests/conformance/
```

## Iron rules

1. No stage may panic on any input. Malformed source produces diagnostics.
2. Every merged PR adds or enables conformance tests. Green suite = definition of correct.
3. The formatter is the AST pretty-printer. There is no second formatting code path.
4. No dependency may be added to the compiler without a PR explaining why (we
   practice the dependency discipline we preach).
