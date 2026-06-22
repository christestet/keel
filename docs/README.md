# Documentation

Keel separates learning material, operational guides, normative specification,
design decisions, and implementation status. This page is the public entry
point; contributor-only reading order remains in [`AGENTS.md`](../AGENTS.md).

## Learn Keel

1. [Getting started](getting-started.md) — build the current source toolchain and
   run, check, format, build, and test a program.
2. [Language tour](language-tour.md) — conformance-backed syntax and semantics
   through M7.
3. [Standard library reference](stdlib-reference.md) — implemented API surface
   and known backend ceilings.
4. [Packages and capabilities](packages-and-capabilities.md) — manifests, local
   dependencies, authority declarations, and audit reports.

## Use the toolchain

- [CLI reference](cli-reference.md)
- [Compiler diagnostics](diagnostics.md)
- [Users-service example](../examples/users-service/README.md)
- [Security policy](../SECURITY.md)

## Understand the language

- [Vision](vision.md) — why Keel exists and the intended complete design.
- [Who Keel is not for](who-keel-is-not-for.md) — explicit domain boundaries.
- [Language specification](spec/) — normative behavior; `keel-core.md` is the
  frozen M0–M4 subset and numbered chapters add later behavior.
- [Keel Decision Records](kdr/) — accepted/rejected decisions and reopening
  evidence.

The conformance suite is the executable specification. If it disagrees with
normative prose, stop and file an issue rather than reconciling either silently.

## Follow implementation

- [Roadmap](../ROADMAP.md) — milestone order and binary exit criteria.
- [Milestone status](milestone-status.md) — current implementation snapshot.
- [Compiler architecture](../compiler/ARCHITECTURE.md) — pipeline, crate layout,
  and compiler invariants.
- [Conformance guide](../tests/conformance/README.md) — executable case format.

Non-normative implementation notes live alongside this file as `*-status.md`,
`*-implementation.md`, and `*-audit.md`. They record work; they do not define
the language.

## Contribute

- [Contributing](../CONTRIBUTING.md)
- [Agent contributor rules](../AGENTS.md)
- [Governance](../GOVERNANCE.md)
