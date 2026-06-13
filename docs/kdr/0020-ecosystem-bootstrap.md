# KDR-0020: Ecosystem bootstrap strategy

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** toolchain, governance

## Decision

Keel's ecosystem bootstraps through three layers that work together:

**Layer 1 — Codegen as ecosystem multiplier.** `keel gen` ships in the core
toolchain. Point it at a `.proto` file, OpenAPI spec, SQL DDL, or JSON Schema
and it emits a fully typed client or server skeleton using only the stdlib.
The explicit goal: Keel "has" a typed Stripe/Kubernetes/gRPC client the moment
the schema exists, without anyone maintaining a third-party package.

**Layer 2 — C FFI as deliberate bridge strategy.** For what schemas cannot
describe (Kafka wire protocol, librdkafka, sqlite, libpq edge cases), Keel
wraps existing battle-tested C libraries. Each official wrapper carries a KDR-
tracked plan stating whether it stays a wrapper or gets a pure-Keel replacement
once usage justifies it.

**Layer 3 — `x.keel.dev`, the extended library.** A single official namespace
with stdlib discipline (same review bar, compatibility promise, security
process) but independently versioned. Kafka, Redis, cloud SDKs, OIDC live
here. Promotion path: community → `x` (at usage threshold) → `std` (only via
edition).

**The patron problem.** Keel's governance is a multi-sponsor foundation from
day one. Every component in `std` or `x` must have a named, paid maintainer of
record before it is admitted. A library nobody is paid to maintain does not
enter the official namespaces.

## Context

Derived from [`docs/vision.md`](../vision.md) §2.

The chicken-and-egg problem kills young languages: no users without libraries,
no libraries without users. Keel's answer is structural, not aspirational.
Codegen converts machine-readable schema into typed libraries at compile time
— the act of publishing a proto spec is equivalent to publishing a Keel client.
FFI bridges the gap for protocols too complex for codegen. The paid-maintainer
rule prevents the slow rot that afflicts unmaintained stdlib surface (Python's
`urllib`, Go's `encoding/xml`).

## Alternatives considered

- **Rely on community packages alone** (npm/Rust/Cargo model). Rejected: slow,
  uneven quality, security surface. Keel's adoption unit is "one new
  microservice in an afternoon" — it must work day one.
- **Thick stdlib** (bundle everything in `std`). Rejected: unsustainable
  maintenance burden, incompatible with the paid-maintainer rule.
- **Sponsor-driven stdlib** (one company builds the adapters). Rejected:
  single-company languages are the failure mode KDR-0001's foundation design
  exists to prevent.

## Consequences

- The day-one "is there a library for X?" answer is structurally different:
  the ecosystem grows with the schema ecosystem, not the Keel community.
- `keel gen` is a core compiler deliverable, not an afterthought. Its output
  quality is a language-quality metric.
- Wrapper maintenance is an explicit cost tracked per-KDR. The plan-to-purify
  clause prevents permanent FFI debt.
- The paid-maintainer requirement constrains `std`/`x` growth to the funding
  envelope — by design.

## Reopening clause

Evidence that (a) the codegen approach misses a material fraction of real-world
backend integration needs, or (b) the paid-maintainer rule has prevented
adoption of a component whose absence measurably slows the ecosystem (measured:
proportion of blocked adoption attempts).
