# KDR-0007: No build scripts; hermetic builds

- **Status:** accepted
- **Scope:** toolchain

## Decision

Keel has no build scripts or compile-time code execution. `keel.toml` is data
only. Builds are hermetic and sandboxed.

## Context

Derived from [`docs/vision.md`](../vision.md) §3. Build scripts are the most
common vector for supply-chain attacks and nondeterministic builds. Hermetic
builds mean `keel build` on untrusted code is safe by definition.

## Reopening clause

None; foundational.
