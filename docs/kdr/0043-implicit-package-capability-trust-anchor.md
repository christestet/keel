# KDR-0043: Implicit packages are the capability trust anchor; `keel audit` reports their derived capabilities

- **Status:** accepted
- **Date:** 2026-07-03
- **Scope:** language

## Decision

Capability **enforcement** (`K1110`/`K1112`, [spec §11.3](../spec/11-capabilities.md))
applies to **explicit packages only** — packages rooted by a `keel.toml`
manifest. An **implicit package** (a single file compiled with no adjacent
manifest, [spec §6.1](../spec/06-modules-packages.md)) is the **trust anchor**
of a build: it can only ever be the compilation root, never a dependency
(a dependency path without `keel.toml` is already `K1106`), so there is no
supply-chain boundary to enforce at it. Its capability set is **derived, not
declared**: the union of the §11.2 obligations of the `std` modules its source
uses. `keel audit` must report that derived set honestly, marked `(derived)` —
it must never report an implicit package as having no capabilities while its
source can exercise them. Spec §6.1's description of the implicit package
("empty capability set") is superseded by this derived-set definition.

## Context

The M7 implementation enforces capabilities for explicit packages and their
dependency graphs, but exempts implicit packages entirely
(`check_workspace` returns early when no `keel.toml` exists —
`compiler/keelc-driver/src/manifest.rs`). This preserved all M0–M6
single-file behavior, and dozens of conformance cases (736–806) depend on
`keelc run main.keel` using `std.http`/`std.sql` without a manifest.

That exemption left two defects:

1. **Spec self-contradiction.** §6.1 defines the implicit package as having an
   "empty capability set" while §11.3 says a package may exercise a capability
   only if declared. Read together they demand rejecting every manifest-less
   file that uses `std.http` — which the implementation deliberately does not
   do, and which the conformance suite forbids. The prime directive says this
   conflict must be resolved by decision, not silently.
2. **A lying audit.** `keel audit` on an implicit package prints every
   capability as `not present` even when the file imports `std.http`. A
   capability report that under-reports authority is worse than no report:
   KDR-0011's entire value is that `keel audit` is exhaustive without running
   the program.

The threat model (KDR-0011, vision §3) is **supply-chain scrutiny of
dependencies**, not protection of the developer from their own entry point.
The person invoking `keelc run main.keel` on a manifest-less file is the
author of that file; a declaration ritual there adds friction without adding
safety, and would break the frozen M0–M6 surface. What must stay ironclad is
the boundary where third-party code enters — and it does: dependencies are
required to carry manifests (`K1106`), so nothing transitive escapes
enforcement.

## Alternatives considered

- **Enforce declarations for implicit packages too** (require a manifest the
  moment `std.http` is used). Rejected: breaks the frozen M0–M6 conformance
  surface (dozens of accept-cases), destroys the single-file teaching/scripting
  path, and adds no supply-chain safety — the implicit package is by
  construction first-party code at the root, never a dependency.
- **Keep the exemption and the current audit output.** Rejected: leaves
  `keel audit` reporting `not present` for authority the program demonstrably
  has. An audit that can under-report is not an audit (KDR-0011).
- **Grant implicit packages the full capability set** (report all six as
  present). Rejected: over-reports instead of under-reports; the derived set
  from actual `std` uses is exactly computable, deterministic, and honest.
- **Deprecate implicit packages entirely.** Rejected: single-file programs are
  the first contact every newcomer has with the language; forcing a manifest
  for `hello.keel` contradicts the compile-time-ergonomics positioning
  (KDR-0021) for zero threat-model gain.

## Consequences

- `keel audit` output for implicit packages changes: derived capabilities are
  listed with a `(derived)` marker instead of the blanket `not present` line.
  This is a behavior change and follows the spec → tests → implementation
  sequence (root AGENTS.md hard rule 1).
- Spec §6.1's "empty capability set" wording must be amended to "derived
  capability set (§11.x)" in the spec PR that encodes this decision.
- Enforcement code paths (`K1110`/`K1112`) are untouched; no existing
  conformance case changes behavior.
- The trust-anchor rule is now explicit: any future feature that would let an
  implicit package be *imported* (rather than be the root) must first
  supersede this KDR, because it would turn the exemption into a real bypass.

## Reopening clause

Reopen if any of the following becomes true:

- A language or toolchain change allows an implicit (manifest-less) package to
  be depended upon by another package — the exemption would then be a
  supply-chain hole, and enforcement (or mandatory manifests) must be
  reconsidered.
- Corpus evidence shows production services shipped as implicit packages at
  meaningful scale (≥5% of corpus services), meaning the trust-anchor
  assumption ("implicit = first-party root, authored by the invoker") no
  longer holds in practice.
- A demonstrated attack uses the implicit-package path to smuggle undeclared
  authority past `keel audit` despite the derived-set reporting.
