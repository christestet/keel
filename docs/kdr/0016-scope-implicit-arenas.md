# KDR-0016: Scope-implicit arenas — region allocation via structured concurrency

- **Status:** proposed
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Every `scope` block implicitly creates an allocation region. Allocations
within a `scope` that the compiler's escape analysis proves do not outlive
the block boundary are served from the scope's region and freed in O(1) on
scope exit, without GC involvement. Explicit `arena` blocks (KDR-0012) remain
available for non-concurrent region allocation; the scope-implicit arena is
the concurrent counterpart, not a replacement.

## Context

Disentanglement research (CMU/PLDI, "Linear Promises" at ECOOP) shows that
structured-concurrency tree lifetimes are the natural unit for hierarchical
memory management: when tasks form a strict tree (via `scope { spawn ... }`),
their allocation graphs are provably acyclic across siblings. This allows the
runtime to reclaim per-task memory in bulk when the task scope exits, rather
than tracing through it.

Keel already has two aligned decisions:
- **Structured concurrency with `scope`/`spawn`** (KDR-0002) — no detached
  tasks, tree-structured lifetimes.
- **Scoped arenas via `arena` blocks** (KDR-0012) — region allocation with
  escape analysis.

This KDR connects them: the same escape analysis KDR-0012 already requires
is applied to `scope` blocks automatically, giving every concurrent region
the same GC-free allocation path that explicit `arena` blocks give to
sequential hot paths. The programmer writes nothing new.

OCaml 5's effect handlers and JVM Project Loom's virtual threads both
demonstrate that structured lifetimes unlock GC optimisations; this KDR makes
it a language guarantee rather than a JIT heuristic.

## Alternatives considered

- **Full hierarchical GC** (generational per-task heaps with minor collections
  on scope exit). Rejected: Keel already has a concurrent low-latency GC plus
  arenas as the escape hatch. Adding a third memory system (hierarchical GC)
  violates KDR-0012's "no second memory paradigm" principle.

- **Manual arena annotation per allocation inside scope** (`arena { ... }`
  wrapping each concurrent section). Rejected: defeats the "default to fast"
  goal. If every concurrent scope is implicitly a region, the fast path is the
  default and the explicit `arena` keyword is only needed in sequential code.

- **Status quo** (GC sees all scope-internal allocations). Rejected: high-
  throughput concurrent services create many short-lived task trees; GC
  pressure from these is the primary performance bottleneck this design
  targets.

## Consequences

- Hot-path concurrent code gets GC-free allocation by default — no new syntax,
  no annotation burden.
- The escape analysis must be extended from `arena` blocks (already planned in
  KDR-0012) to also check `scope` boundaries. This is the same analysis, new
  target rule: references may not escape their containing `scope`.
- Allocations that the analysis cannot prove are scope-local fall through to
  the GC — this is correct, just potentially suboptimal (optimisation surface).
- Explicit `arena` blocks inside a `scope` nest their regions as sub-regions
  (freed upon arena exit, not scope exit).

## Reopening clause

Corpus evidence that the escape analysis scope check causes significant false
positives in real-world concurrent code (allocations proven to escape a scope
that a human judge agrees are actually scope-local), reducing the benefit
below the complexity cost of maintaining the additional analysis target.
