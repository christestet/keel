# KDR-0009: No operator overloading or implicit conversions

- **Status:** accepted
- **Scope:** language

## Decision

Keel has no operator overloading and no implicit type conversions. Operators
are built-in only (see KDR-0013). Mixing types — e.g. `Int + Float` — is a
compile error (`K0202`); convert explicitly.

## Context

Derived from [`docs/vision.md`](../vision.md) §1, Appendix A. Operator
overloading lets libraries change the meaning of basic syntax, making code
unreadable across project boundaries. Implicit conversions hide bugs.

## Reopening clause

None; foundational.
