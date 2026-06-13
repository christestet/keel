# KDR-0019: Compile time as a contract

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** toolchain

## Decision

The build budget is a versioned, public, CI-enforced artifact. The reference
benchmark (a realistic 100k-LOC service suite, in the open) must satisfy:

- Cold build: **< 10 s**
- Incremental build: **< 1 s**
- `keel check` (types + lint, no codegen): **< 300 ms**

CI treats a regression beyond 5% as a release blocker, equal in severity to a
miscompilation. The compiler is architected around incrementality from day one
(salsa-style query-based core), not retrofitted.

## Context

Derived from [`docs/vision.md`](../vision.md) §7.

The failure mode is incremental: SQL checking, exhaustiveness, capability
verification, and structural-diff assertions are each individually fast, and
their sum is how a fast compiler becomes a slow one (Rust's most expensive
lesson). Keel targets the team iteration loop; compile time is a UX property,
not a performance detail.

The 5% regression gate prevents slow accumulation. The public benchmark
prevents "works on my machine" dismissals. The salsa-based query architecture
is specified as the target from day one (compiler/ARCHITECTURE.md) — this KDR
bakes the requirement into the release criteria.

## Alternatives considered

- **No fixed budget** ("we'll make it fast eventually"). Rejected: every
  language that succeeded at compile-time performance baked it into the
  architecture early (Go, OCaml) — every language that didn't, regretted it
  (Rust, C++).
- **Growth allowance** (budget scales linearly with corpus size). Rejected:
  Keel's niche (backend services) has bounded complexity per service.
  100k-LOC is generous for a single service; the budget holds at that line
  count.

## Consequences

- The compiler's query-based architecture (salsa) is not optional — it is a
  release gate.
- SQL and schema checking must be aggressively cached (keyed on migration set
  hash, re-checked only when input text changes).
- Features that interact poorly with incrementality (e.g., global analysis
  passes) require explicit KDR-level justification.

## Reopening clause

Evidence from the reference corpus that the budget is either unachievable with
the required feature set, or so easily achievable that it provides no signal.
