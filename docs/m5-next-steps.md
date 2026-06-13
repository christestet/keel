# M5 next-work note

Non-normative hand-off for the next coding session. The governing documents are
[`AGENTS.md`](../AGENTS.md), [`docs/vision.md`](vision.md),
[`docs/spec/keel-core.md`](spec/keel-core.md), [`ROADMAP.md`](../ROADMAP.md),
and [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md).

## Status

M4 is closed:

- `keel build|run|test|fmt` all work.
- Conformance cases cover `run`, `test`, and `build` modes.
- Go build cache is shared across conformance runs via `target/gocache`.

M5 has not started.

## Dependency chain

- M5 scope: [`ROADMAP.md`](../ROADMAP.md) §M5 — interfaces, user generics,
  `scope`/`spawn` structured concurrency.
- Governing KDRs:
  - [`docs/kdr/0003-no-inheritance.md`](kdr/0003-no-inheritance.md) — interfaces
    (≤5 methods), no inheritance.
  - [`docs/kdr/0002-no-async-await.md`](kdr/0002-no-async-await.md) — no
    async/await; structured concurrency only.
  - [`docs/kdr/0012-gc-plus-scoped-arenas.md`](kdr/0012-gc-plus-scoped-arenas.md)
    and [`docs/kdr/0016-scope-implicit-arenas.md`](kdr/0016-scope-implicit-arenas.md)
    — `scope` blocks and resource/arena scoping.
- Spec chapter plan: [`docs/spec/00-spec-plan.md`](spec/00-spec-plan.md).
- Architecture: [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md).

## Milestone boundary

Do **not** touch M6 (stdlib slice / `std.http` / `std.json` / SQL) or M7
(capabilities, `keel audit`, `arena`, `keel gen`, editions) until M5 is closed.
Do not extend the parser or backend for features that are not yet spec'd.

## Validation snapshot

```sh
# Default preflight (M1 gate)
scripts/preflight.sh
# → 89 passed, 0 failed, 4 skipped

# M4 milestone gate
KEEL_MILESTONE=M4 scripts/preflight.sh
# → 93 passed, 0 failed, 0 skipped
```

## Next work

1. **Draft spec chapter `docs/spec/07-interfaces.md`**
   - Governed by KDR-0003.
   - Cover interface declaration, explicit `impl`, method calls, ≤5-method
     enforcement, and stable `K####` codes.
   - This must land as its own PR before tests or implementation (hard rule 1).

2. **Add conformance cases for interfaces**
   - Accept cases: define an interface, `impl` it for a struct, call through the
     interface, pass/return interfaces.
   - Reject cases: missing method, wrong signature, >5 methods.
   - Validate with `cargo run -p conformance-runner -- --check`.

3. **Implement interfaces end-to-end**
   - Parser (`keelc-parse`), typechecker (`keelc-resolve` / `keelc-types`),
     Go backend (`keelc-backend-go`), and formatter round-trip.
   - PR description must list the conformance cases it makes pass.

4. **Repeat spec → tests → implementation for user generics**
   - Spec chapter `docs/spec/08-generics.md`.
   - Syntax: `fn foo<T: Interface>(x: T)`.
   - Interface-constrained only.

5. **Repeat for `scope` / `spawn`**
   - Spec chapter `docs/spec/09-concurrency.md`.
   - `scope { spawn task() }`; tasks cannot outlive the scope.
   - Map to goroutines in the Go backend.

## Notes

- The formatter is the AST pretty-printer. Any new syntax must round-trip
  through `keel fmt`.
- Every compiler PR must keep the conformance suite green and reference the
  cases it makes pass.
- `keel test` structural diffs are a future enhancement; Core currently only
  requires pass/fail + source line.
