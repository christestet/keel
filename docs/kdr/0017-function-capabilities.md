# KDR-0017: Function-level capability annotations

- **Status:** proposed
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Functions may optionally declare `use capabilities(net, fs, ...)` in their
signature. The compiler verifies call-graph transitively: a function without
`net` may not call anything (including through FFI) that opens a network
connection. Package-level capability declarations (KDR-0011) remain the
mandatory first gate; function-level annotations add finer granularity for
auditing and enforcement, and may only restrict, never grant, what the
package declares.

## Context

Package-level capabilities (KDR-0011) answer the supply-chain question: "which
of my dependencies can touch the network?" This is sufficient for left-pad
scenarios. For compliance and security regimes (SOX, PCI-DSS, SOC2, internal
security reviews), teams need the finer question: "which specific call paths
can touch PII-bearing network endpoints or the filesystem?"

Function-level annotations make `keel audit --verbose` produce a per-function
capability report — a one-screen answer to "the encryption-key loader is the
only function in this service that needs `fs`." This composes with the waiver
system (KDR-0018): a function with broad capability use carries a discoverable,
accountable waiver trail.

This is deliberately NOT a linear-capability / token-passing system (Austral,
Pony, Rust's owned types). There is no runtime token, no ownership transfer,
no resource linearity — purely static call-graph analysis at compile time.
This keeps the annotation optional (you opt in when compliance demands it) and
compatible with KDR-0012's "no ownership/lifetimes" decision.

## Alternatives considered

- **Lineage-capability tokens** (Austral/Pony style: resource handle is a
  linear type passed as function argument). Rejected: requires linear types
  and ownership semantics, which KDR-0012 explicitly excludes. The learning
  curve and signature overhead are antithetical to the "five-year team" goal.

- **Package-level only** (status quo, KDR-0011). Rejected: compliance use cases
  require per-function audit trails. A package containing 50 handler functions
  should not require the same audit scrutiny for every function when only 3
  touch the network.

- **Runtime permission checks** (capability check on each system call).
  Rejected: defeats the compile-time guarantee. The whole point is that
  `keel audit` is exhaustive without running the program.

## Consequences

- Regulated teams get fine-grained audit trails without changing their
  deployment or adding external tooling.
- Annotation is fully optional — unannotated functions inherit the package
  capability by default (no migration burden on existing code).
- Function-level annotations can only restrict, never expand, the package
  declaration. This preserves the package as the authoritative trust boundary.
- Conflicts with reflection/macros — consistent with KDR-0004 (no reflection,
  no macros), so the call graph is always fully visible to the compiler.

## Reopening clause

Corpus evidence that the annotation burden on packages with legitimate broad
capability use outweighs the compliance value, or that in practice the
package-level declaration (KDR-0011) satisfies all real-world audit
requirements without function-level granularity.
