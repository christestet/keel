# KDR-0006: No conditional compilation beyond OS/arch

- **Status:** accepted
- **Scope:** language

## Decision

Keel has no feature flags or conditional compilation beyond the built-in OS/arch
target predicates. Prevents $2^n$ untested configuration combinations.

## Context

Derived from [`docs/vision.md`](../vision.md) §1, Appendix A.

## Reopening clause

None; foundational.
