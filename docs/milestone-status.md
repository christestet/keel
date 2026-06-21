# Milestone status

Non-normative implementation status for the current milestone-based build-out.
The governing language definition is [`docs/spec/keel-core.md`](spec/keel-core.md);
the executable spec is [`tests/conformance/`](../tests/conformance/).
For milestone scope and exit criteria, see [`ROADMAP.md`](../ROADMAP.md).

## M1 — Frontend: lexer, parser, AST, diagnostics

| Area | State |
|---|---|
| Span crate (`keelc-span`) | Source IDs, byte spans, spanned values, line/column mapping. |
| Diagnostics crate (`keelc-diag`) | Diagnostic types, stable codes, append-only code registry. |
| Lexer (`keelc-lex`) | Tokenizes Keel Core, newline-terminated, reports `K0102`, `K0906`, `K0004`. |
| AST (`keelc-ast`) | Declarations, types, blocks, statements, expressions, match arms, patterns. |
| Parser (`keelc-parse`) | Modules, declarations, function signatures, types, blocks, expressions, match/catch, tests, rejection forms with stable diagnostics. |
| Driver (`keelc-driver`) | `keelc check <file>` — reads one source file, runs lex+parse, emits `error[K####]` / `warning[K####]` with spans. |
| Conformance runner | Defaults to M1 syntax validation, invokes `keelc check`. |
| KDR surface | KDR-0013 (operators) and KDR-0014 (interpolation brace doubling `{{`/`}}`) lex/parser support integrated. |

**Excluded:** resolver, typechecker, KIR, Go backend, `keel` CLI, formatter beyond AST pretty-printer.

## M2 — Semantic analysis: resolver + typechecker

| Area | State |
|---|---|
| Resolver (`keelc-resolve`) | Immutable assignment and required struct fields with defaults. |
| Type checker | Primitive arithmetic/comparison typing, local `let` inference, `if/else` arm compatibility, string interpolation, selected built-in calls. |
| Exhaustive match | Reports `K0402` for missing enum and built-in `Option`/`Result` variants. |
| `?` typing | Reports `K0501` when enclosing return type cannot absorb propagated `Result`/`Option`. |
| `catch` typing | Reports `K0502` for non-exhaustive error handling. |
| Constructor typing | `Some`, `None`, `Ok`, `Err`, enum variants, `checked_div`, `checked_rem` — temporary type info for the current conformance surface. |

**Not done:** no typed-HIR crate yet; shared type info extracted into `keelc-types` but typechecker still lives in `keelc-resolve`. Generic constructor typing uses `TypeInfo::Unknown` as scaffolding (not the final unification model). Pattern exhaustiveness is whole-variant only, not a general matrix. `K0503` registered but not yet covered by conformance.

## M3 — Go backend

| Area | State |
|---|---|
| Go runtime shim (`keelc-backend-go`) | `KeelEnum`, `Some`/`None`, `Ok`/`Err`, `checked_div`, `checked_rem`, div/rem-by-zero panics with `K0204`. |
| Structs | Declarations, literals, field defaults, nested struct field access. |
| Enums and payloads | Tagged values with payload storage and constructor functions; enum variants can hold struct payloads. |
| `match` | Match expressions and statement-position matches — wildcard arms, guards, payload bindings. |
| `Option` / `Result` | M3 tagged representation. |
| `?` | `let value = expr?` lowers to temporary + early `return`. |
| `catch` | `let value = expr catch err { ... }` — success extraction, matched error arms, `other` fallback. |
| Driver (`keelc-driver`) | `keelc check` and `keelc run`; `run` cleans up its temporary Go build directory. |
| **Conformance score** | **91 / 91 passed** (M3 milestone). Exit criterion met. |

**Known failures:** none.

**Done:** `keelc-kir` crate introduced; backend emits from explicitly-typed KIR.
The resolver/typechecker is the single expression-inference owner and KIR
consumes its span-keyed type results. Shared `TypeContext` declaration tables
remain in `keelc-types`. See
[`docs/m6-simplification-audit.md`](m6-simplification-audit.md).

## M4 — Toolchain skeleton: CLI, fmt, test

| Area | State |
|---|---|
| `keel` / `keelc` binaries | Split `keelc-driver` into a library plus `keel` and `keelc` binary shims. |
| `keel fmt` | Implemented via the AST pretty-printer; idempotent on all M0 accept cases in `tests/conformance/`. |
| `keel build` | Compiles a Keel source file to a native binary via the Go toolchain; artifact is placed next to the source file. |
| `keel test` | Discovers `test "name" { ... }` blocks, runs each in an isolated Go harness, reports assertion failures with source line. |
| Conformance | New cases `702-keel-test-runs-blocks` and `801-keel-build-produces-binary` exercise `keel test` and `keel build` at M4. |

**Known limitations:** formatter strips comments (comments are not stored in the AST). `examples/users-service/main.keel` uses post-Core features and cannot be formatted yet.

## M5 — Language completion wave 1

| Area | State |
|---|---|
| Interfaces | Implemented end-to-end: [`docs/spec/07-interfaces.md`](spec/07-interfaces.md) is ratified, parser/typechecker/Go backend/formatter support nominal interfaces with explicit `impl`. |
| `interface` / `impl` | `interface Name { fn method(self) -> T }` and `impl Interface for Type { ... }` parse, typecheck, and lower to Go interfaces and receiver methods. |
| Method calls | `receiver.method(args)` resolves through explicit impls or interface declarations; dynamically dispatched through interface values. |
| Interface types | Interface names may be used as parameter/return/local types; concrete values assigned to interface types require a matching `impl`. |
| Diagnostics | `K0601`–`K0607` registered and emitted for interface/impl violations. |
| Formatter | `keel fmt` round-trips interface and impl syntax. |
| Conformance | Cases `212-interface-declaration` through `222-impl-extra-method` exercise accept and reject behavior. |

**User generics (done):** [`docs/spec/08-generics.md`](spec/08-generics.md) is
implemented end-to-end. Type parameters are represented as `TypeInfo::TypeParam`
and erased to their bound interface in Go; structs satisfy bounds through their
receiver methods and primitive `impl`s become `keelBox_<Prim>` wrapper types
(Go cannot attach methods to predeclared types). The typechecker emits `K0802`
(method outside the bound, at the definition site) and `K0803` (type argument
fails structural constraint satisfaction, at the call site). Cases `223`–`233`
pass at M5. See [`docs/generics-implementation.md`](generics-implementation.md).

**Structured concurrency (partially done):** [`docs/spec/09-concurrency.md`](spec/09-concurrency.md)
is implemented for the initial M5 executable slice. The compiler parses
`scope`/`spawn`, formats the syntax, types `Task<T>`, enforces `K0701`-`K0703`,
lowers through KIR, and emits Go with a join barrier, `context.WithCancel`,
`sync.WaitGroup`, and deterministic first-error selection by spawn order.
Conformance cases `710`-`715` pass at M5, and `903-no-scope-spawn` is gated
through M4.

**Remaining concurrency work:** `scope(deadline: ...)` parses and lowers, but Go
backend deadline emission is intentionally still unsupported until M6 provides
the time/duration stdlib surface. Explicit cancellation checkpoint APIs such as
`check_cancel()` are specified but not exposed by the current stdlib slice.

**Generics parser (done):** `TypeParam` AST node with `name`, `bound`, `span`;
`type_params` on `FunctionDecl`, `StructDecl`, `EnumDecl`; `type_args` on `Expr::Call`,
`Expr::StructLiteral`, `ImplDecl`. Parser accepts `[T: Bound]` on functions, structs,
enums, and impls, and `[T, U]` on calls and struct literals — all gated to milestone
≥5. Diagnostic codes K0801–K0807 registered; K0801/K0804/K0805/K0806 emitted by parser;
K0802/K0803 by the typechecker; K0807 is subsumed by K0601. Pretty printer round-trips
all generic syntax.

**Known limitations:** interfaces are limited to ≤5 methods (KDR-0003); no default methods, inheritance, or structural subtyping.

## M6 — Stdlib slice (in progress)

| Area | State |
|---|---|
| `std.time` / deadline / `check_cancel` | Done. `time.Duration`, `time.milliseconds`, `time.seconds`, `time.sleep`, `check_cancel()`, `scope(deadline: …)`. Conformance cases `716`–`723` pass. See [`docs/spec/15-stdlib-core.md §15.1–15.4`](spec/15-stdlib-core.md). |
| `std.json` | Done. `json.parse[T]`, `json.write(value)`, strict and tolerant modes, struct/enum/Option/primitive codecs, formatter round-trip, `K1503` compile-time guard. Cases `724`–`735` pass. See [`docs/m6-json-implementation.md`](m6-json-implementation.md). |
| `std.http` | Done. `http.serve`, `http.Request`/`Response`, 7 response constructors, `http.Error(BindFailed)`, `K1504`/`K1505`. Cases `736`–`745` pass. See [`docs/spec/15-stdlib-core.md §15.17–15.24`](spec/15-stdlib-core.md) and [`KDR-0028`](kdr/0028-http-server-surface.md). |
| `std.sql` | Not started. |
| `std.log` | Done. `log.info`, `log.warn`, `log.error` — simple string messages to stdout. Cases `746`–`748` pass. See [`docs/spec/15-stdlib-core.md §15.25–15.27`](spec/15-stdlib-core.md). |
| `std.config` | Not started. |
| `examples/users-service/main.keel` | Not compilable yet (requires `std.config`, `std.sql`, multi-line strings). M6 exit criterion. |

**Conformance score:** 152 / 152 passed at M6, 2 skipped (M4-only cases).

## Future: LSP server (M7+)

| Area | State |
|---|---|
| KDR | [`KDR-0103`](kdr/0103-lsp-server.md) — proposed. Defers LSP work to M7+; requires salsa query core. |
| Spec | [`docs/spec/16-lsp.md`](spec/16-lsp.md) — landed. Capability table, diagnostics mapping, server lifecycle. |
| Crate | Not started. Depends on `tower-lsp` + `tokio`; blocked on salsa-style incrementality (target architecture). |

**Not started:** no `keelc-lsp` crate, no `keel lsp` subcommand, no workspace
state management. The salsa query core is a prerequisite — see
[`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) §Query-based core.

## Milestone key

| # | Title | Exit criterion |
|---|---|---|
| M0 | Core + conformance suite | ≥60 cases, accept and reject, reviewed and frozen |
| M1 | Frontend | Every M0 case parses or fails with correct `K####` |
| M2 | Semantic analysis | All M0 reject-cases produce exact codes; accept-cases typecheck |
| M3 | Go backend | `keelc run` passes 100% of M0 accept-cases; `examples/hello.keel` works |
| M4 | CLI + fmt + test | `keel test` runs Keel-language tests; `keel fmt` idempotent on repo |
| M5 | Interfaces + generics + concurrency | Language completion wave 1 |
| M6 | Stdlib slice | `examples/users-service/` compiles and runs |
| M7 | Differentiators | Package capabilities, arenas, `keel gen`, editions machinery |

## Validation

Run `scripts/preflight.sh` from the repo root. For a specific milestone:

```sh
KEEL_MILESTONE=M3 scripts/preflight.sh
# or equivalently:
cargo run -p conformance-runner -- --keelc target/debug/keelc --milestone M3
```

M4 adds `keel test` execution; validate with:

```sh
KEEL_MILESTONE=M4 scripts/preflight.sh
```

M5 adds interfaces, generics, and the initial `scope`/`spawn` implementation;
validate with:

```sh
KEEL_MILESTONE=M5 scripts/preflight.sh
# → 119 passed, 0 failed, 2 skipped
```

M6 adds `std.time`, `std.json`, and structured concurrency time/deadline APIs;
validate with:

```sh
KEEL_MILESTONE=M6 scripts/preflight.sh
# → 139 passed, 0 failed, 2 skipped
```

## Dependency chain

1. [`AGENTS.md`](../AGENTS.md) — global agent rules and definition of done
2. [`docs/vision.md`](vision.md) — language and tooling rationale
3. [`docs/spec/keel-core.md`](spec/keel-core.md) — frozen M0-M4 language subset
4. [`docs/kdr/INDEX.md`](kdr/INDEX.md) — accepted decisions
5. [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) — pipeline, crate layout, iron rules
6. [`ROADMAP.md`](../ROADMAP.md) — milestone boundaries
7. Relevant conformance cases in [`tests/conformance/`](../tests/conformance/)

Compiler-local rules: [`compiler/AGENTS.md`](../compiler/AGENTS.md)  
Spec rules: [`docs/spec/AGENTS.md`](spec/AGENTS.md)  
KDR rules: [`docs/kdr/AGENTS.md`](kdr/AGENTS.md)  
Conformance rules: [`tests/conformance/AGENTS.md`](../tests/conformance/AGENTS.md)
