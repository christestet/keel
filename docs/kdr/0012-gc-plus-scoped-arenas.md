# KDR-0012: GC + scoped arenas; no ownership/lifetimes

- **Status:** accepted
- **Scope:** language

## Decision

Keel uses a concurrent low-latency GC for general allocation, plus lexically
scoped `arena` blocks for hot-path allocation that bypasses the GC. There is no
manual `free`, no lifetime annotation syntax, no ownership/borrowing model. The
compiler's escape analysis forbids arena references from outliving the block.

## Context

Derived from [`docs/vision.md`](../vision.md) §4. Arena blocks are region
allocation with the same shape as `scope` blocks: lexical, visible, impossible
to leak, and checked. This solves the "GC escape hatch" question without
introducing a second memory paradigm.

## Reopening clause

Evidence of significant real-world workloads that arenas + GC cannot serve.
