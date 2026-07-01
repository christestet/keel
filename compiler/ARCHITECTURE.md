# keelc — Compiler Architecture

## Implementation language: Rust ([KDR-0101](../docs/kdr/0101-compiler-in-rust.md))

Chosen for: memory-safe compiler internals, excellent error-handling ergonomics
for diagnostics, `salsa` for query-based incremental compilation, mature parser
and IR tooling, and single-binary distribution. Self-hosting Keel-in-Keel is a
post-1.0 aspiration, not a plan.

## Pipeline

### Current (M8 partial)

```
source --> lexer --> parser --> AST --> resolver/typechecker --> KIR --> backend
             |          |                    |                  |             |
             +----------+-----  diagnostics (stable K#### codes)  ------------+
```

### Target

```
source --> lexer --> parser --> AST --> resolver --> typechecker --> KIR --> backend
             |          |                    |             |                    |
             +----------+-----  diagnostics (stable K#### codes)  -------------+
```

- **Query-based core ([salsa](https://github.com/salsa-rs/salsa)-style) is the
  active architecture.** Every stage is designed as a memoized query keyed on
  inputs. This is how the [vision.md §7](../docs/vision.md#7-compile-time-as-a-contract)
  budget (incremental < 1s, `keel check` < 300ms) stays achievable. The salsa
  integration lives in `keelc-query` — its own crate, not inside `keelc-driver`,
  so `keel lsp` (in `keelc-lsp`) and the `keel` CLI (in `keelc-driver`) can both
  depend on it without a cycle (`keelc-driver` depends on `keelc-lsp` for the
  `lsp` subcommand). `keel check/run/test/build` and `keel lsp` route through
  the same `keelc-query::SourceFile` input plus parse, resolve, typecheck, KIR
  lowering, backend-emission, and diagnostic queries. Retrofitting
  incrementality is the single most expensive mistake a compiler project makes
  (see: rustc).
- **Lexer/parser:** hand-written recursive descent. Parser must recover from
  errors (parse the whole file, report many diagnostics, never crash).
- **AST → name resolution + type checking:** `keelc-resolve` performs name
  resolution and type checking directly on the AST. Explicit function
  signatures; inference is local (`let`) only. Exhaustiveness checking is part
  of typechecking, not a lint. The typechecker returns a deterministic map of
  expression spans to types. `TypeContext` in `keelc-types` holds declaration
  tables shared with KIR lowering; expression inference has one owner in
  `keelc-resolve`.
- **KIR (Keel IR):** small, explicitly-typed, desugared IR (e.g. `?` and
  `catch` become explicit match-and-return). `keelc-kir` lowers the AST to KIR
  using the typechecker's expression-type map, and all backends consume KIR.
- **Backends:** `backend-go` first ([KDR-0102](../docs/kdr/0102-go-backend-first.md)) — emits readable Go, drives the Go
  toolchain for codegen, GC, scheduler, cross-compile, static linking.
  `backend-native` (LLVM or cranelift) replaces it later; the conformance suite
  is the equivalence proof.

## Diagnostics are a public API

Every error has a stable code `K####`, a primary span, and a "what to do" line.
Conformance reject-tests assert on codes. Changing a code is a breaking change.
Error message *text* may improve freely.

## Crate layout

```
compiler/
  keelc-span        source maps, spans, file ids
  keelc-diag        diagnostic types, K#### registry (registry is a checked-in file)
  keelc-lex         lexer
  keelc-parse       parser -> AST
  keelc-ast         AST definitions (+ pretty printer = the formatter's core)
  keelc-resolve     name resolution + typechecker (operates directly on the AST)
  keelc-types       type definitions (TypeInfo, merge, collect) and shared
                    declaration tables/queries (TypeContext)
  keelc-kir         IR + lowering (AST -> KIR)
  keelc-backend-go  Go emission (consumes KIR)
  keelc-query       Salsa query core (M8, KDR-0106): SourceFile input plus
                    parse/resolve/typecheck/lower/emit/diagnostic queries,
                    shared by keelc-driver and keelc-lsp
  keelc-driver      CLI entry; owns the side-effect boundary (filesystem,
                    process execution); builds both the user-facing `keel`
                    binary (including its `lsp` subcommand) and the `keelc`
                    binary used by the conformance runner
  keelc-lsp         M8 base LSP server — protocol handlers, workspace state,
                    capability table, invoked by `keel lsp`; see
                    [KDR-0103](../docs/kdr/0103-lsp-server.md) and
                    [spec ch. 16](../docs/spec/16-lsp.md)
  conformance-runner  test harness for tests/conformance/
```

## Cross-cutting gotchas

Hard-won invariants that span crates. Ignoring these produces either a
non-compiling Rust workspace or invalid generated Go — both caught late.

- **A new `TypeInfo` variant ripples across exhaustive matches.** `TypeInfo`
  lives in `keelc-types`, but `Display for TypeInfo`, the backend `go_type`,
  and `zero_value` (in `keelc-backend-go`) match it exhaustively. Add the
  variant, then let `cargo build` walk you to every site — don't hand-search.
  Resolver inference and KIR type consumers mostly match specific arms with an
  `_` fallback, so *also* audit those: a silent `_ => Unknown` is how a new
  type gets dropped.
- **Expression-type map keys must identify the complete AST node.** Postfix
  nodes such as `value?` include their postfix token in their span; otherwise
  the node collides with its operand and overwrites its type. Interpolation
  types use the interpolation's outer source span. See
  [`docs/m6-simplification-audit.md`](../docs/m6-simplification-audit.md).
- **Go forbids methods on predeclared types** (`int64`, `string`, …). Any time
  Keel attaches behaviour to a primitive (interface `impl`s, generic bounds),
  the backend must box it into a defined wrapper type. See the `keelBox_<Prim>`
  pattern and erasure strategy in
  [`docs/generics-implementation.md`](../docs/generics-implementation.md).
- **`expr_ty(Name)` is `Unknown` in `keelc-backend-go`.** The backend does not
  re-derive the type of a bare name. Codegen that must know a variable's static
  type (e.g. to box it) currently can't for `Name`s — only literals carry a
  type. Widen this deliberately, with a conformance case, when a feature needs
  it; don't assume it already works.
- **Type parameters are erased, not monomorphized.** `TypeInfo::TypeParam` is
  kept distinct only through checking (so `K0802`/`K0803` can fire) and lowers
  to its bound interface for emission. A future feature that needs per-
  instantiation specialization (e.g. generic-over-primitive arithmetic) does
  not yet have a code path — that's a design decision to revisit, not a bug to
  patch around.

## Iron rules

1. No stage may panic on any input. Malformed source produces diagnostics.
2. Every merged PR adds or enables conformance tests. Green suite = definition of correct.
3. The formatter is the AST pretty-printer. There is no second formatting code path.
4. No dependency may be added to the compiler without a PR explaining why (we
   practice the dependency discipline we preach).
