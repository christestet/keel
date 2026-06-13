# KDR-0011: Package capabilities (net/fs/exec/ffi)

- **Status:** accepted
- **Scope:** language

## Decision

Every package manifest must declare what it touches: `net`, `fs`, `exec`, `env`,
`ffi`, `unsafe-memory`. The compiler enforces the declaration transitively — a
package without `net` cannot reach the socket API. `keel audit` produces a
one-screen answer to "which of my dependencies can open a network connection?"

## Context

Derived from [`docs/vision.md`](../vision.md) §3. Package capabilities make
micro-dependencies low-risk by construction and concentrate scrutiny on packages
with broad capability requests.

## Reopening clause

Evidence that the capability set is insufficient to model real supply-chain
threats, or that the annotation burden on packages with legitimate broad access
causes measurable friction.
