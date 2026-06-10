# KDR-0101: keelc is written in Rust

- **Status:** accepted
- **Scope:** toolchain

## Decision
The compiler and toolchain are written in Rust. Self-hosting is a post-1.0
aspiration, never a milestone dependency.

## Context / Alternatives
Go (rejected: weaker enum/pattern-matching makes compiler internals — ASTs, IRs,
exhaustive lowering — markedly more error-prone; ironically the exact gap Keel
fixes). OCaml/Haskell (rejected: ideal fit technically, too small a contributor
pool, and LLM agents produce measurably better Rust). C++ (rejected on safety).
Rust gives memory-safe internals, salsa for query-based incrementality,
cranelift/LLVM bindings, single-binary distribution, and the largest pool of
compiler-experienced contributors and agent training data.

## Reopening clause
None foundational; a future self-hosting KDR may supersede after 1.0.
