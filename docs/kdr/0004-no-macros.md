# KDR-0004: No macros or compile-time metaprogramming

- **Status:** accepted
- **Scope:** language

## Decision
Keel has no macros, no compile-time code execution, no reflection, no
annotations. The sanctioned alternatives are a rich stdlib and `keel gen`
(schema-driven code generation, visible as checked-in or build-listed output).

## Context
Macros are how codebases grow private dialects: every large Rust/Lisp/Scala
codebase requires learning its local macro layer first, defeating "every repo
looks the same." Reflection enables mocking frameworks, DI containers, and
annotation magic — the abstraction debt Keel exists to prevent. This will be
the single most requested feature; this KDR is the permanent answer.
KDR-0008, which separately considered reflection, was folded into this KDR;
the rejection of reflection is subsumed by the decision above.

## Alternatives considered
Hygienic declarative macros only (rejected: still dialect-forming). Comptime à
la Zig (rejected: powerful but creates a second language inside the language).
Reflection without macros (rejected: runtime magic is worse than compile-time).

## Consequences
Some boilerplate is permanent and visible. Serialization/ORM ergonomics are
solved by compiler-built-ins (`json.parse<T>`, SQL row checking) and codegen,
not by user libraries — concentrating that power in the spec, where it is
reviewed once.

## Reopening clause
Corpus analysis showing ≥ a defined threshold of real Keel code hand-writing
the same structural boilerplate that a corpus-tested macro design demonstrably
eliminates without dialect formation (measured: cross-repo idiom divergence).
