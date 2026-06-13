# KDR-0005: No exceptions

- **Status:** accepted
- **Scope:** language

## Decision

Keel has no exception mechanism. `Result<T, E>` + `?` + `catch` handle all
recoverable errors. Panics are uncatchable across task boundaries.

## Context

Derived from [`docs/vision.md`](../vision.md) §1. Exceptions create invisible
control flow, make local reasoning impossible, and defeat the five-year
readability goal. `Result` types make error paths explicit and matchable.

## Reopening clause

None; foundational.
