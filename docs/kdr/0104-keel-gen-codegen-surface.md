# KDR-0104: `keel gen` — schema-driven codegen in the core toolchain

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** toolchain

## Decision

`keel gen` is a **core toolchain command** that turns a machine-readable schema
into fully typed Keel source. It supports **protobuf and OpenAPI first**
(SQL DDL, JSON Schema, AsyncAPI are named follow-ons), and from a `.proto` or
OpenAPI document it emits a typed client and server skeleton that uses **only
the standard library** — generated code carries **no generated-code runtime
dependency**.

The generated output:

- is **deterministic** — byte-identical for identical input and generator
  version (root [`AGENTS.md`](../../AGENTS.md) hard rule 7);
- **round-trips `keel fmt`** — it is ordinary Keel source, reviewable and
  diffable, not an opaque artifact;
- **declares the capabilities it needs** in its package manifest
  ([`KDR-0011`](0011-package-capabilities.md)) like any other code — a generated
  network client declares `net`, nothing is implicit;
- introduces **no new compiler or runtime dependency** into the consuming
  project.

`keel gen` is an explicit command, never an implicit build step — it does not
run during `keel build` ([`KDR-0007`](0007-no-build-scripts.md): no build
scripts). Generated files are checked in and evolve under review like any source.

## Context

From [`docs/vision.md`](../vision.md) §2, "layer one: codegen ecosystem
multiplier" — the single highest-leverage bootstrap decision in the design. Most
backend work is described by a machine-readable schema; if the toolchain emits a
typed client/server the moment a schema exists, Keel effectively "has" a typed
client for Stripe, Kubernetes, or any internal gRPC service without anyone
maintaining a package for it. The first hundred integrations are *generated, not
written*, side-stepping the npm-style micro-dependency maintenance treadmill.

This KDR records the **surface** decision that [`KDR-0020`](0020-ecosystem-bootstrap.md)
(ecosystem bootstrap) left to elaboration: which schemas, what output shape, and
the determinism / capability / dependency constraints on generated code. It is
the foundation for the `keel init service --from proto` landing kit
([`docs/vision.md`](../vision.md) §9).

## Alternatives considered

- **Hand-written client packages per service** (npm/cargo status quo). Rejected:
  reintroduces exactly the micro-dependency maintenance burden the capability
  and codegen story exists to avoid. A generated client has no maintainer to
  abandon it.

- **Codegen as a build-time plugin / build script.** Rejected: violates
  [`KDR-0007`](0007-no-build-scripts.md). Generation is an explicit, auditable
  command whose output is reviewed source, not a hidden compile-time effect.

- **Third-party / out-of-core codegen** (protoc plugins, openapi-generator).
  Rejected: the multiplier only works if codegen ships *in the core toolchain*
  with stdlib-only output — a third-party generator fragments quality and
  reintroduces a dependency.

- **Opaque generated artifacts** (binary descriptors, non-formatted output).
  Rejected: generated code that does not round-trip `keel fmt` is unreviewable
  and undiffable, defeating the audit posture.

## Consequences

- Keel "has" a typed client for any schema the moment that schema exists; the
  integration cost drops to running one command.
- Generated code is ordinary, formatted, capability-declared Keel — reviewable,
  diffable, and subject to the same supply-chain audit as hand-written code.
- Determinism makes regenerated output a clean diff, so schema drift is visible
  in code review rather than hidden in a binary.
- The `keel init service --from proto` landing kit and the broader bootstrap
  strategy ([`KDR-0020`](0020-ecosystem-bootstrap.md)) build directly on this
  surface.
- Each new schema format is additive work in the toolchain, not a change to the
  language.

## Reopening clause

Evidence that in-core codegen cannot keep pace with the churn of real schema
formats, or that generated-output quality for a major format demands a plugin
model that the core toolchain cannot absorb without compromising the stdlib-only
and determinism guarantees.
