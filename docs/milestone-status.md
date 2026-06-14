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

**Done:** `keelc-kir` crate introduced; backend emits from explicitly-typed KIR. Shared `TypeContext` in `keelc-types` supplies type information to both `keelc-resolve` and KIR lowering.

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

**Remaining M5 work:** user generics ([`docs/spec/00-spec-plan.md`](spec/00-spec-plan.md) chapter 08) and `scope`/`spawn` structured concurrency (chapter 09).

**Known limitations:** interfaces are limited to ≤5 methods (KDR-0003); no default methods, inheritance, or structural subtyping.

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

M5 adds interfaces; validate with:

```sh
KEEL_MILESTONE=M5 scripts/preflight.sh
# → 103 passed, 0 failed, 1 skipped
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
