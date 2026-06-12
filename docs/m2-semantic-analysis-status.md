# M2 Semantic Analysis Status

Small wiki note for the current M2 resolver/typechecker work. This note is
non-normative; the governing language definition remains
[`docs/spec/keel-core.md`](spec/keel-core.md), and the executable spec remains
[`tests/conformance/`](../tests/conformance/).

## Status

Implemented in the current M2 compiler work:

| Area | State |
|---|---|
| Resolver | [`compiler/keelc-resolve`](../compiler/keelc-resolve/) checks immutable assignment and required struct fields with defaults. |
| Type checking | The checker handles primitive arithmetic/comparison typing, local `let` inference, `if/else` arm compatibility, string interpolation expressions, and selected built-in calls. |
| Exhaustive match | The checker reports `K0402` for missing enum and built-in `Option`/`Result` variants. |
| `?` typing | The checker reports `K0501` when the enclosing function return type cannot absorb the propagated `Result` error or `Option` absence. |
| `catch` typing | The checker reports `K0502` for non-exhaustive enum error handling unless an `other` or wildcard fallback is present. |
| Constructor typing | `Some`, `None`, `Ok`, `Err`, enum variants, `checked_div`, and `checked_rem` have enough temporary type information to satisfy the current M2 conformance surface. |

Not done yet:

- There is no typed HIR crate yet; type information is still local to
  `keelc-resolve`.
- Generic constructor typing uses `TypeInfo::Unknown` as a temporary stand-in
  for proper unification. This keeps current `Some`/`None` and `Ok`/`Err`
  branches type-compatible, but it is not the final type system architecture.
- Pattern exhaustiveness is intentionally shallow. It covers whole-variant
  enum, `Option`, and `Result` cases in the current conformance suite; it is not
  a general pattern matrix/checker.
- `K0503` union-error match exhaustiveness is registered but not yet covered by
  a conformance case in this implementation slice.

## Dependency Chain

Read order for this work:

1. [`AGENTS.md`](../AGENTS.md): global rules and definition of done.
2. [`docs/vision.md`](vision.md): language and tooling rationale.
3. [`docs/spec/keel-core.md`](spec/keel-core.md): frozen M0-M4 language subset.
4. [`docs/kdr/INDEX.md`](kdr/INDEX.md): accepted decisions and KDR stubs.
5. [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md): compiler pipeline,
   crate layout, and iron rules.
6. [`ROADMAP.md`](../ROADMAP.md): M2 milestone boundary.
7. Relevant conformance cases in [`tests/conformance`](../tests/conformance/).

Relevant compiler-local rules:

- [`compiler/AGENTS.md`](../compiler/AGENTS.md)
- [`tests/conformance/AGENTS.md`](../tests/conformance/AGENTS.md), when touching
  conformance cases

## Governing Behavior

The implementation follows existing Core behavior only:

| Source | Constraint |
|---|---|
| [`keel-core.md`](spec/keel-core.md) | `match` is exhaustive and reports `K0402` for missing variants. |
| [`keel-core.md`](spec/keel-core.md) | `Option<T>` and `Result<T, E>` are built-in generic types in Core. |
| [`keel-core.md`](spec/keel-core.md) | `?` unwraps `Ok`/`Some` or propagates `Err`/`None`; bad return contexts report `K0501`. |
| [`keel-core.md`](spec/keel-core.md) | `catch` must cover the error type or end in a propagating fallback arm, reporting `K0502` otherwise. |
| [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) | Exhaustiveness checking belongs in typechecking, not parser recovery. |
| [`ROADMAP.md`](../ROADMAP.md) | M2 includes resolver/typechecker work only; backend behavior remains M3. |

## Milestone Boundary

M2 includes:

- name resolution
- local type inference
- explicit function signature checking
- struct construction checks
- `Option`/`Result`, `?`, and `catch` typing
- `match` exhaustiveness

M2 excludes:

- KIR lowering
- Go backend code generation
- runtime behavior for accept-cases
- full typed-HIR architecture
- full pattern exhaustiveness engine

Do not use the temporary `TypeInfo::Unknown` compatibility behavior as a reason
to accept underspecified type rules. It is implementation scaffolding only.

## Validation Snapshot

Latest local validation for this M2 semantic slice:

```text
cargo run -p conformance-runner -- --keelc target/debug/keelc --milestone M2
suite ok: 90 case(s), structure valid
88 passed, 0 failed, 2 skipped

scripts/preflight.sh
harness: ok
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p conformance-runner -- --check
suite ok: 90 case(s), structure valid
cargo build --release -p keelc-driver
cargo run -p conformance-runner -- --keelc target/release/keelc
88 passed, 0 failed, 2 skipped
preflight: green
```

The two skipped cases are M3 runtime panic cases:

- `121-int-div-by-zero-panics`
- `122-int-rem-by-zero-panics`

## Next Work

Concrete follow-ups:

1. Replace the temporary generic `Unknown` merge with principled unification in
   the typed-HIR/typechecker boundary.
2. Add conformance coverage for `K0503` before implementing union-error match
   exhaustiveness.
3. Grow exhaustiveness checking from whole-variant coverage into the pattern
   checker needed by future pattern forms, without changing Core behavior.
4. Keep backend/runtime work out of this M2 slice; M3 owns execution and Go
   lowering.
