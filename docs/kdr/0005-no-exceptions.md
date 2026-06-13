# KDR-0005: No exceptions

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Keel has no exception mechanism. `Result<T, E>` + `?` + `catch` handle all
recoverable errors. Panics are uncatchable across task boundaries. Union error
types (`-> Result<User, DbError | ParseError>`) provide composition without
exception inheritance.

## Context

Derived from [`docs/vision.md`](../vision.md) §1. Exceptions create invisible
control flow, make local reasoning impossible, and defeat the five-year
readability goal. `Result` types make error paths explicit and matchable.

The union error type (`A | B`) fills the composition gap that leads languages
to create exception hierarchies: instead of `throws IOException` with subtype
polymorphism, a function declares exactly which error types it produces, and
the caller must handle each or propagate explicitly.

Algebraic effects (OCaml 5, Koka) were considered and rejected per KDR-0002:
research-grade learning curve, violates positioning. `Result` + union types
achieve the same "typed error tracking" goal with less conceptual machinery.

## Alternatives considered

- **Checked exceptions** (Java). Rejected: still invisible control flow (the
  exception propagates up the call stack without being visible at the call
  site), signature pollution (`throws` clauses grow monotonically), and the
  "catch or declare" pattern encourages swallowing.

- **Unchecked exceptions** (Go's panic/recover, C++/Python/Ruby). Rejected:
  uncatchable panics are simpler: if you cannot recover, you terminate the
  task. Recoverable errors are `Result` types. Two mechanisms for two distinct
  categories.

- **Algebraic effects** (OCaml 5, Koka, Unison). Rejected per KDR-0002:
  delimited continuations and effect handlers require a significant learning
  investment that conflicts with Keel's positioning as a readable,
  replaceable-team language.

- **Monad transformers / error monads** (Haskell). Rejected: the ergonomic
  cost of composing multiple monad transformers is prohibitive for backend
  code, exactly the problem the user description identified.

## Consequences

- Every possible error path is visible in the function signature. Code review
  can verify error handling without knowing the runtime call graph.
- `?` provides the same "early return on error" brevity as exceptions for the
  common case (propagate), while `catch` provides exhaustive local handling.
- Panics are truly exceptional: logic bugs, assertion failures, invariants.
  They are not a control-flow mechanism.
- Union error types must be matched exhaustively (`K0503`), preventing the
  "unknown how to handle this" class of runtime bugs.

## Reopening clause

None; foundational.
