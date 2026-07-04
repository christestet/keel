# Compatibility policy

Keel is pre-1.0. The published `0.1.x` line (`v0.1.0`, `v0.1.1`) is a
**developer preview**, not production infrastructure. This document distinguishes
the compatibility mechanisms already enforced by the repository from the
long-term promises in the language design; the preview's honest scope and limits
are the [Developer-preview scope](#developer-preview-scope-and-limits) section
below.

## Current support level

Only the active development branch is maintained. Milestone snapshots, old
commits, locally copied binaries, and generated artifacts receive no backports.
The source language and CLI may change through the repository's KDR → spec →
conformance → implementation process before 1.0.

No change may silently contradict an accepted KDR, normative specification, or
passing conformance case. That process is the current compatibility control; it
is not semantic-versioning stability for users.

`0.1.x` is described as a developer preview; a stronger support window would
require a separate compatibility KDR. Do not infer production support, package
ecosystem stability, or backports from the `0.1.x` version number.

## Language compatibility

[`keel-core.md`](spec/keel-core.md) is frozen for M0–M4. Later language behavior
is normative only where a numbered spec chapter and conformance cases have
landed. Implementation behavior without those contracts is not stable API.

Edition 1 is the only recognized edition. A package may declare:

```toml
[package]
edition = "1"
```

Omitting the field selects the toolchain's current edition, also edition 1.
Unknown editions are `K1401`.

The accepted long-term policy is that old editions continue to compile and a
new edition cannot ship without a complete mechanical `keel fix` migration.
Neither promise has been exercised yet because there is no second edition and
`keel fix` is not implemented. `K1402` and `K1403` are reserved accordingly.

## Diagnostic compatibility

`K####` codes are append-only public identifiers. Existing codes are never
renumbered or reused. Tools should match the code and severity, not exact
message/help text, because human wording may improve.

Source spans and deterministic ordering are compiler contracts. A missing span,
wrong code, or unstable order is a defect.

See the [diagnostic catalogue](diagnostics.md).

## Formatting compatibility

`keel fmt` is the only formatter and has no style options. Its output is locked
by formatter/conformance tests and must be idempotent. Before 1.0, canonical
layout may still change through an intentional formatter concern, but two styles
will not be supported concurrently as configuration choices.

## Standard-library compatibility

The conformance-backed M6 surface is implemented, but there is no released
stdlib versioning policy yet. Signatures and wire mappings in normative chapter
15 cannot change through an implementation-only PR.

Behavior mentioned only as aspirational—such as structured log fields—or
specified without backend/conformance coverage—such as raw HTTP header/query
access—is not a compatibility promise.

## CLI compatibility

The current command shape and `--milestone M<N>` flag are development
scaffolding. There is no stable CLI compatibility guarantee before a released
toolchain. In particular, `--milestone` is expected to disappear from normal
user workflows once the active language surface becomes the default.

Exit statuses and commands are documented as current behavior in the
[CLI reference](cli-reference.md), not as a 1.0 commitment.

## Package compatibility

Package names and versions currently serve identity and diagnostics only.
Dependencies are local paths; there is no registry resolver, lockfile, package
compatibility solver, or publication policy. Do not infer SemVer dependency
selection from the required `version = "MAJOR.MINOR.PATCH"` spelling.

The capability set and audit ordering are normative. Current implicit packages
bypass capability enforcement, so their behavior must not be treated as a
stable security guarantee.

## Generated source and binaries

`keel gen` output is deterministic for identical schema input and generator
version. Output may change between compiler versions when the normative mapping
changes; generated source should be checked in so that change is reviewable.

Binary reproducibility is verified for a fixed conformance input under one
toolchain environment. Keel does not yet promise a stable binary ABI, artifact
format, cross-version linking, or cross-host byte identity.

## Before 1.0

A 1.0 compatibility declaration still needs explicit decisions for:

- supported host/target platforms and release lifetime;
- compiler/stdlib version coupling;
- package version resolution and lockfiles;
- stable CLI behavior;
- native backend artifact and ABI expectations;
- security update and backport windows.

Those policies must be decided before release rather than inferred from this
development implementation.

## Developer-preview scope and limits

`0.1.x` is a developer preview. Its conformance-backed surface is listed in
[`feature-status.md`](feature-status.md); its release mechanics are governed by
[`release-process.md`](release-process.md).

Honest limitations that must stay prominent in any preview presentation:

- **Go toolchain required.** Executable generation still shells out to `go build`;
  the native backend is M11. SQL cases may resolve `modernc.org/sqlite` from a
  module cache or the network.
- **Incremental build = whole-build cutoff only.** An unchanged `keel build`
  is a verified no-op (a stamp beside the output binary records the source,
  compiler, and Go-toolchain inputs), which is what the enforced
  `keel_build_incremental` budget measures. An *edited* source still reruns
  the full pipeline: the query database is fresh per invocation, and
  per-module reuse across invocations is future work (see
  [`milestone-status.md`](milestone-status.md) §M8).
- **LSP is base-capability only.** `keel lsp` resolves module-level `fn`/`struct`
  declarations by name; parameters and `let` bindings (local scopes) are not
  indexed yet.
- **Not in `0.1.x`:** package registry/publishing, native backend, reproducible
  OCI image output, C FFI, OpenAPI client/server generation, dependency
  lockfiles/version resolution, and any production support/backport window.
