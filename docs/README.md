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
5. [Idiomatic Keel](idiomatic-keel.md) — non-normative patterns derived from the
   design and conformance suite.

## Use the toolchain

- [CLI reference](cli-reference.md)
- [Compiler diagnostics](diagnostics.md)
- [Deployment](deployment.md)
- [Troubleshooting](troubleshooting.md)
- [Users-service example](../examples/users-service/README.md)
- [Security policy](../SECURITY.md)

## Understand the language

- [Vision](vision.md) — why Keel exists and the intended complete design.
- [Who Keel is not for](who-keel-is-not-for.md) — explicit domain boundaries.
- [Syntax/specification index](syntax-index.md) — map from language forms to
  normative prose and executable cases.
- [Language specification](spec/) — normative behavior; `keel-core.md` is the
  frozen M0–M4 subset and numbered chapters add later behavior.
- [Keel Decision Records](kdr/) — accepted/rejected decisions and reopening
  evidence.

The conformance suite is the executable specification. If it disagrees with
normative prose, stop and file an issue rather than reconciling either silently.

## Follow implementation

- [Roadmap](../ROADMAP.md) — milestone order and binary exit criteria.
- [Feature status](feature-status.md) — concise implemented/partial/planned
  matrix for users.
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

## Releases and compatibility

- [Compatibility policy](compatibility.md)
- [0.1.0 release readiness](0.1-release-readiness.md)
- [Changelog](../CHANGELOG.md)
- [Release process](release-process.md)

## Documentation integrity

Every local file or section reference must be a Markdown link, not an
unlinked path in prose. Public documentation must be reachable from the root
[`README.md`](../README.md), normally through this index. Run
[`scripts/check-docs.sh`](../scripts/check-docs.sh) to reject broken file links,
broken section anchors, self-links, and orphan documents. The same check runs in
CI and as part of [`scripts/preflight.sh`](../scripts/preflight.sh).

Navigation backlinks are allowed: an index must be able to link to a guide that
links back to its governing index. Dependency descriptions should link toward
their governing spec, KDR, architecture, conformance, and roadmap sources rather
than restating those sources.
