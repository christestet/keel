# KDR-0007: No build scripts; hermetic builds

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** toolchain

## Decision

Keel has no build scripts or compile-time code execution. `keel.toml` is data
only. Builds are hermetic and sandboxed.

## Context

Derived from [`docs/vision.md`](../vision.md) §3. Build scripts are the most
common vector for supply-chain attacks (npm `preinstall`/`postinstall` scripts,
PyPI setup.py) and nondeterministic builds (time, network access, filesystem
side-effects during compilation). Hermetic builds mean `keel build` on untrusted
code is safe by definition.

Code generation is handled by `keel gen` (KDR-0020) as an explicit, user-invoked
step, not a build-time side effect. The output of `keel gen` is checked-in or
build-listed source, not a build-time dependency.

## Alternatives considered

- **Limited build-script API** (Rust `build.rs` — a compile-time Rust program
  with restricted capabilities). Rejected: even restricted, build scripts
  execute arbitrary user code at build time. The history of `build.rs`
  (network access during compilation, filesystem probes, non-deterministic
  output) shows that restricted APIs are insufficient in practice.

- **No guard at all** (npm model — arbitrary code runs at install time).
  Rejected: the most exploited supply-chain vector in the industry. `preinstall`
  scripts are how left-pad's successors exfiltrate credentials.

- **Sandboxed build scripts** (sandboxing the build-time execution
  environment). Rejected: sandboxes are leaky (Rust's proc-macro sandbox
  bypasses are a recurring CVE category). The only safe script is one that
  does not run.

## Consequences

- `keel build` on untrusted code (CI artifacts, community registry packages)
  is safe by default — no code executes during compilation.
- Code generation is always explicit: `keel gen` is a separate step with its
  own output that the developer reviews and commits.
- Some build-time optimisations (C FFI linking configuration, platform-
  specific library paths) must be expressed as data in `keel.toml`, not as
  code. The manifest format must be expressive enough for common cases.
- Cross-compilation is deterministic: the build output depends only on source
  + manifest data, not on the build machine state.

## Reopening clause

Corpus evidence that the absence of build scripts prevents building a class of
packages that the ecosystem requires, and that the required functionality
cannot be expressed through `keel.toml` data keys or `keel gen` plugins.
