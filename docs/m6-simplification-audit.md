# M6 Simplification Audit and Refactor Ledger

Status: complete on branch `m6` at merge commit `27716be`. This is a
non-normative implementation note. It records a behavior-preserving cleanup;
it does not change the Keel language.

## Governing documents

- [`docs/spec/keel-core.md`](spec/keel-core.md), especially §5 for `?` and
  `catch` semantics
- [`KDR-0033`](kdr/0033-universal-error-type.md) for `Error` absorption at `?`
- [`KDR-0037`](kdr/0037-sql-error-classification-patterns.md) for catch
  propagation
- [`KDR-0038`](kdr/0038-union-narrowing-patterns.md) for typed patterns
- [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) for stage ownership
- [`ROADMAP.md`](../ROADMAP.md) for the M6 boundary
- [`tests/conformance/`](../tests/conformance/) for executable behavior
- [`scripts/preflight.sh`](../scripts/preflight.sh) for the definition of done

## Outcome

The audit found repeated work rather than a need for new abstractions. The
cleanup removed 1,162 lines and added 263, for a net reduction of 899 lines
across eleven files. It also removed two backend-only dependency edges.

The resulting semantic path is:

```text
AST -> resolver/typechecker -> diagnostics + expression types -> KIR -> Go
```

The resolver/typechecker is now the only expression-inference implementation.
KIR lowering consumes its results and retains `TypeContext` only for shared
declaration tables and queries such as enum payloads and exhaustive variants.

## Decisions

### One inference owner

The large resolver was not split merely because it is large. It owns
diagnostic-aware inference and the conformance behavior. The separate,
diagnostic-free inference engine in `keelc-types` duplicated its rules and was
the actual drift risk, so that duplicate was deleted.

`TypecheckOutput` now carries both diagnostics and a deterministic
`BTreeMap<Span, TypeInfo>`. `Span` derives `Ord` so it can be the map key. The
driver typechecks once and passes the same output to `run`, `test`, and `build`;
KIR does not re-infer expressions.

### The `?` span decision

Expression types are keyed by AST span. Previously, postfix `value?` reused
the exact span of `value`. That made the operand and the enclosing
`Expr::Question` collide in the type map: recording the question result could
overwrite the operand's `Result` or `Option` type. KIR needs both values when it
desugars `?` into an explicit temporary, tag test, payload extraction, and
early return.

The parser now joins the operand span with the consumed `?` token. Therefore:

```text
value   -> span covering `value`
value?  -> span covering `value?`
```

This is a source-location invariant, not a language-semantics change. The
existing `?` rules remain governed by Core §5 and KDR-0033. It fixed five
conformance regressions exposed while switching KIR to typechecker results.

### Interpolation spans

Interpolation expressions are parsed from the string fragment after the main
AST is built. Their synthetic expression spans are not stable keys in the
outer source map, so the typechecker explicitly records the inferred type at
the interpolation's outer source span. KIR uses that outer span when lowering
the interpolation.

### Missing type entries

KIR lookup falls back to `TypeInfo::Unknown`. This is defensive handling for a
missing internal entry, not a second inference path. User-visible type errors
are still produced by the resolver/typechecker before lowering.

### Backend feature detection

The Go backend previously walked the complete KIR once per feature question.
Those scans were replaced by one visitor that builds `ModuleUsage`. It tracks
the same runtime/import flags, including whether a use occurs in a struct
default. No cache, trait hierarchy, or dependency was added.

### Tests and dependencies

Six backend unit tests duplicated existing conformance cases. They were
deleted, and the backend's now-unused direct dependencies on `keelc-parse` and
`keelc-span` were removed. The conformance suite remains the behavioral test
boundary.

### Smaller deletions

- Removed a redundant test-emitter constructor.
- Removed unused span APIs while retaining the span operations used by the
  pipeline.
- Removed an unused diagnostic lookup API.
- Shared the existing task type helpers instead of keeping resolver and KIR
  copies.
- Unified the driver's Go emission path for `run`, `test`, and `build`.
- Unified temporary Go execution for normal programs and tests.
- Removed KIR's obsolete local scopes, bindings, and current-return state after
  KIR stopped inferring types.
- Removed 611 lines of unreachable diagnostic-free inference from
  `keelc-types`.

## Deliberately not changed

- No spec, KDR, diagnostic code, conformance case, or language behavior changed.
- The resolver/typechecker remains one crate; splitting it without a semantic
  boundary would only move code.
- `TypeContext` remains in `keelc-types` for shared declaration information.
- No salsa/query database was introduced; that remains target architecture.
- The Go runtime was not split just to reduce file size.
- Linear declaration tables remain; current module sizes do not justify new
  indexing machinery.
- No dependency was added.

## Incremental commits

| Commit | Change |
|---|---|
| `e7eb308` | Remove redundant test emitter constructor |
| `62a7652` | Remove unused span APIs |
| `1c6af28` | Remove unused diagnostic lookup |
| `377f156` | Share task type helpers |
| `d80c330` | Share Go emission pipeline |
| `5812fda` | Share Go run path |
| `e4c5204` | Collect backend usage in one pass |
| `4c2d90b` | Retain typechecker expression types |
| `75fd7a3` | Rely on backend conformance coverage |
| `7021963` | Lower with typechecker results; distinguish postfix `?` span |
| `e0118b8` | Remove lowering scope inference state |
| `dbe6431` | Remove duplicate KIR type inference |
| `27716be` | Merge `refactor/semantic-dedup` into `m6` |

## Validation snapshot

Final command:

```sh
KEEL_MILESTONE=M6 scripts/preflight.sh
```

Result on 2026-06-21:

```text
harness: ok
cargo fmt: ok
clippy --workspace --all-targets -D warnings: ok
cargo test --workspace: ok
conformance: 185 passed, 0 failed, 3 skipped
preflight: green
```

The skipped cases (`901`–`903`) are intentionally valid only through M4.

## Next work

Continue M6 from [`docs/milestone-status.md`](milestone-status.md) and
[`ROADMAP.md`](../ROADMAP.md). Add another inference path only if a new stage
cannot consume `TypecheckOutput`; otherwise extend the existing typechecker and
add the governing conformance case first.
