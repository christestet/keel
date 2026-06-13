# KDR-0011: Package capabilities (net/fs/exec/ffi)

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Every package manifest must declare what it touches: `net`, `fs`, `exec`, `env`,
`ffi`, `unsafe-memory`. The compiler enforces the declaration transitively — a
package without `net` cannot reach the socket API. `keel audit` produces a
one-screen answer to "which of my dependencies can open a network connection?"

FFI specifically: `extern` blocks are the only door, they require the `ffi`
capability, every crossing appears in the audit report and the SBOM, and
`extern` code is excluded from Keel's safety guarantees with a mandatory
documented contract (what the C side may do with each pointer).

## Context

Derived from [`docs/vision.md`](../vision.md) §3. Package capabilities make
micro-dependencies low-risk by construction and concentrate scrutiny on packages
with broad capability requests. A left-pad-style utility package that requests
`net` is visibly absurd before anyone reads its source — this addresses the
npm-culture rejection structurally rather than by registry policy.

The capability set (`net`, `fs`, `exec`, `env`, `ffi`, `unsafe-memory`) is
chosen to match the attack surface a dependency can expose. Each capability
corresponds to a distinct system-call category that a supply-chain attacker
would exploit. Function-level capability annotations (KDR-0017) may extend
this to finer granularity for compliance use cases.

## Alternatives considered

- **No capability system** (trust model — cargo/npm/gem status quo). Rejected:
  the npm left-pad incident and every subsequent supply-chain attack demonstrate
  that the trust model fails. Without structural enforcement, micro-dependencies
  are a security liability.

- **Runtime permission checks** (Android permissions, JVM SecurityManager).
  Rejected: bypasses the compile-time guarantee. The whole point is that
  `keel audit` is exhaustive without running the program.

- **Only net/fs** (omit exec, env, ffi, unsafe-memory). Rejected: incomplete
  threat model. `exec` covers command injection and cryptocurrency miners,
  `env` covers credential exfiltration, `ffi` covers C-library vulnerabilities,
  `unsafe-memory` covers deliberate memory unsafety.

- **Sandbox-only enforcement** (OS-level sandboxing as the sole mechanism).
  Rejected: no compile-time audit trail, no SBOM integration, harder to
  inspect in CI.

## Consequences

- Supply-chain risk becomes visible as a single `keel audit` command output.
- Micro-dependencies are safe by construction (empty capabilities = provably
  harmless).
- `extern` blocks concentrate FFI risk in explicit, auditable locations.
- High-capability packages (anything with `net` + `ffi`) receive focused
  scrutiny by default.
- The SBOM is a natural output of capability traversal, not a separate
  generation step.
- Function-level capability annotations (KDR-0017) can layer on top without
  changing the package-level contract.

## Reopening clause

Evidence that the capability set is insufficient to model real supply-chain
threats, or that the annotation burden on packages with legitimate broad access
causes measurable friction.
