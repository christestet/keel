# M3 Go Backend Status

Small wiki note for the current M3 backend work. This note is non-normative;
the governing language definition remains [`docs/spec/keel-core.md`](spec/keel-core.md),
and the executable spec remains [`tests/conformance/`](../tests/conformance/).

## Status

Implemented in the current M3 compiler work:

| Area | State |
|---|---|
| Go runtime shim | [`compiler/keelc-backend-go`](../compiler/keelc-backend-go/) emits `KeelEnum`, `Some`/`None`, `Ok`/`Err`, `checked_div`, `checked_rem`, and div/rem-by-zero panics with `K0204`. |
| Structs | Core struct declarations, literals, field defaults, nested structs, and field access lower to Go. |
| Enums and payloads | Core enum variants lower to tagged values with payload storage and constructor functions. |
| `match` | Match expressions and statement-position matches lower to Go closures, including wildcard arms, guards, and payload bindings. |
| `Option` / `Result` | Built-in generic values lower to the M3 tagged representation used by the conformance suite. |
| `?` | Statement-position `let value = expr?` lowers to a temporary plus early `return` for `Err` or `None`. |
| `catch` | Statement-position `let value = expr catch err { ... }` lowers to success extraction or matched error handling, including `other` fallback. |

Not done yet:

- There is still no `keelc-kir` crate. The backend emits from AST using a
  backend-local type environment. This satisfies M3 conformance but does not yet
  match the final architecture boundary where all backends consume KIR only.
- The backend-local `TypeInfo` is implementation scaffolding, not the language's
  final typed HIR or unification model.
- Match lowering is scoped to the Core pattern forms currently covered by
  conformance. It is not a general pattern matrix implementation.
- The M4 `keel` CLI, formatter, and test runner remain future work.

## Dependency Chain

Read order for this work:

1. [`AGENTS.md`](../AGENTS.md): global rules and definition of done.
2. [`docs/vision.md`](vision.md): language and tooling rationale.
3. [`docs/spec/keel-core.md`](spec/keel-core.md): frozen M0-M4 language subset.
4. [`docs/kdr/0102-go-backend-first.md`](kdr/0102-go-backend-first.md): Go backend first.
5. [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md): intended pipeline and KIR boundary.
6. [`ROADMAP.md`](../ROADMAP.md): M3 milestone boundary and validation command.
7. Relevant conformance cases in [`tests/conformance`](../tests/conformance/).

Relevant compiler-local rules:

- [`compiler/AGENTS.md`](../compiler/AGENTS.md)
- [`tests/conformance/AGENTS.md`](../tests/conformance/AGENTS.md), when touching
  conformance cases

## Milestone Boundary

M3 includes:

- lowering runnable Core programs to Go
- driving the Go toolchain through `keelc run`
- passing all M0 accept and reject conformance cases at `--milestone M3`
- keeping `examples/hello.keel` runnable

M3 excludes:

- the final KIR/typed-HIR architecture
- the M4 `keel build|run|fmt|test` single-binary UX
- structural test diffs and test discovery
- post-Core features such as interfaces, user generics, `scope`/`spawn`,
  `arena`, capabilities, FFI, and the stdlib slice

## Validation Snapshot

Latest local validation for this M3 backend slice:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p conformance-runner -- --keelc target/debug/keelc --milestone M3
90 passed, 0 failed, 0 skipped

KEEL_MILESTONE=M3 scripts/preflight.sh
90 passed, 0 failed, 0 skipped
preflight: green
```

Use `KEEL_MILESTONE=M<N>` when validating a milestone claim through preflight;
the runner accepts the same value as `--milestone M<N>`.

## Next Work

Concrete follow-ups:

1. Replace backend-local AST type inference with the typed-HIR/KIR boundary
   required by `compiler/ARCHITECTURE.md`.
2. Keep new language behavior behind the spec -> conformance -> implementation
   sequence; do not grow the M3 backend beyond Core to make examples compile.
3. Start M4 separately: `keel` CLI skeleton, formatter path, and `keel test`
   discovery/execution.
