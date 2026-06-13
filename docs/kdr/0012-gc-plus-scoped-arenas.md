# KDR-0012: GC + scoped arenas; no ownership/lifetimes

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Keel uses a concurrent low-latency GC for general allocation, plus lexically
scoped `arena` blocks for hot-path allocation that bypasses the GC. There is no
manual `free`, no lifetime annotation syntax, no ownership/borrowing model. The
compiler's escape analysis forbids arena references from outliving the block.

Arena blocks are region allocation with the same shape as `scope` blocks:
lexical, visible, impossible to leak, and checked. The escape analysis is the
same analysis the GC already needs, pointed at a new rule — it costs no new
conceptual machinery. Scope-implicit arenas (KDR-0016) extend this principle to
concurrent code: every `scope` block is also a region allocation.

## Context

Derived from [`docs/vision.md`](../vision.md) §4. The "GC escape hatch" question
is answered without introducing a second memory paradigm. Hot paths — parsers,
request-scoped object graphs, caches rebuilt per tick — get deterministic,
GC-invisible allocation; everyone else never types the keyword.

The container-awareness promise from vision.md extends: the runtime reads cgroup
limits, and `keel build` can emit a runtime profile (`--profile latency` vs.
`--profile throughput`) that tunes GC pacing at build time — not a 40-variable
tuning surface.

## Alternatives considered

- **Full ownership/borrowing** (Rust model). Rejected: the learning curve and
  annotation burden are antithetical to Keel's "five-year team" goal. Lifetime
  annotations, borrow checker fights, and ownership refactoring cost are
  acceptable in systems programming but unacceptable for backend services.

- **Manual memory management** (C, Zig model). Rejected: defeats the
  productivity goal. Backend services should not require `free` discipline.

- **GC only, no arenas** (Go, Java, C# status quo). Rejected: no escape hatch
  for hot paths. Vision.md anticipates real-world workloads where GC pacing is
  insufficient — parsers and request-scoped graphs are the concrete examples.

- **Reference counting only** (Swift, Python model). Rejected: refcount cycles
  require a cycle-detecting GC anyway. RC overhead on concurrent data
  (atomic increment/decrement on every shared pointer clone) imposes a
  measurable cost on high-throughput services.

- **Region-based only, no GC** (Cyclone, ML Kit model). Rejected: not ergonomic
  for general-purpose backend code. Every allocation would need a region
  annotation or default region, which introduces the same annotation burden as
  lifetimes.

## Consequences

- GC handles the common case; `arena` blocks handle hot paths. Most developers
  never see the GC.
- The escape analysis is a compiler requirement from day one. This analysis
  also enables scope-implicit arenas (KDR-0016) later.
- No lifetime annotations, no `'a`, no borrow checker. Memory safety without
  the learning cliff.
- `arena` syntax is lexically scoped and visually consistent with `scope`,
  `if`, `match` — the language's standard block structure.
- Build-time GC profiles (`--profile latency`, `--profile throughput`) make GC
  tuning a compile-time choice, not a runtime operational burden.

## Reopening clause

Evidence of significant real-world workloads that arenas + GC cannot serve.
