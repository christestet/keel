# KDR-0006: No conditional compilation beyond OS/arch

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Keel has no feature flags or conditional compilation beyond the built-in OS/arch
target predicates. Prevents $2^n$ untested configuration combinations.

## Context

Derived from [`docs/vision.md`](../vision.md) §1. Conditional
compilation is the primary mechanism by which codebases accumulate untested
configurations: every combination of feature flags multiplies the state space,
and CI typically tests only the default configuration. The failure mode is
well-known from C/C++ (`#ifdef` hell) and Rust (`cfg(test)`, `cfg(feature = "...")`
proliferation — every large Rust codebase has dead or semi-maintained feature
gates).

Keel's ecosystem strategy (KDR-0020) removes the main legitimate use case for
feature flags: optional functionality is provided by separate packages in
`x.keel.dev` or the community, not by compile-time feature gating within a
single package.

## Alternatives considered

- **Cargo-style feature flags** (Rust model). Rejected: $2^n$ configuration
  space, combinatorial test burden, and ecosystem fragmentation (tokio vs.
  async-std is a feature-flag schism). Corpus evidence shows most feature flags
  across the Rust ecosystem are either unused or untested outside the default.

- **Preprocessor-style** (`#ifdef` / `#if`, C/C++ model). Rejected: creates
  untestable code paths, makes code harder to read (the same function can mean
  different things on different platforms), and prevents reliable static
  analysis.

- **OS/arch predicates only** (Rust's `cfg(target_os)` / `cfg(target_arch)`).
  Accepted. These are necessary for backend portability (endianness, signal
  handling, filesystem paths) and the set of targets is small and explicit.

## Consequences

- Every compiled configuration is a tested configuration. CI tests the same
  binary that ships.
- Platform differences must be abstracted behind interfaces and separate
  packages, not flags. This encourages proper abstraction boundaries.
- Some patterns (e.g., debug-only assertions without runtime cost) cannot use
  conditional compilation; they use debug-mode runtime checks or separate
  test binaries instead.
- The Go backend (KDR-0102) handles OS/arch conditional emission via Go build
  tags, transparent to Keel source.

## Reopening clause

None; foundational.
