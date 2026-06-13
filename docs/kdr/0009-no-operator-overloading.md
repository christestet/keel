# KDR-0009: No operator overloading or implicit conversions

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Keel has no operator overloading and no implicit type conversions. Operators
are built-in only (see KDR-0013). Mixing types — e.g. `Int + Float` — is a
compile error (`K0202`); convert explicitly.

## Context

Derived from [`docs/vision.md`](../vision.md) §1, Appendix A. Operator
overloading lets libraries change the meaning of basic syntax, making code
unreadable across project boundaries. A `+` that means "append to buffer" in
one library and "encrypt" in another is the opposite of the five-year
readability goal. Implicit conversions hide bugs: every language that has them
has a class of bugs caused by unexpected coercion (JavaScript's `==`, C++'s
single-argument constructors, Go's untyped constants edge cases).

Keel's "one way to do things" principle means `+` is always arithmetic
addition. Domain-specific operations use named functions — this is explicit,
grepable, and reviewable.

## Alternatives considered

- **Limited trait-based overloading** (Rust `std::ops` model — overload `Add`,
  `Mul`, etc. for user types). Rejected: while more disciplined than C++, Rust
  still encourages DSL-like types (newtypes with operator overloads) that
  obscure the meaning of basic syntax across codebases. The reopening clause
  for KDR-0004 (macros) covers the same concern.

- **Full overloading** (C++ model). Rejected: operator semantics become a
  project-specific convention. Reading unfamiliar C++ code requires knowing
  which types overload what.

- **Implicit conversions with restrictions** (Kotlin implicit receiver, Swift
  implicit bridging). Rejected: any implicit conversion is a potential bug
  source. Explicitness is cheap and prevents a well-known bug class.

## Consequences

- `+`, `-`, `*`, `/` have fixed, documented semantics (KDR-0013). Reading code
  never requires checking which overload is in scope.
- Domain types (vectors, matrices, monetary amounts) use named methods
  (`.add()`, `.scale()`) instead of operators. This is more verbose and
  intentionally so — operations on custom numeric types deserve explicit names.
- String concatenation uses `{expr}` interpolation syntax, not `+` — avoiding
  the Java/C# confusion of `+` meaning both addition and concatenation.
- The compiler never performs silent type coercion. `Float.from(i)` is the
  required bridge.

## Reopening clause

None; foundational.
