# KDR-0044: Cross-package symbol linking — dependency source compiles into the root program

- **Status:** accepted
- **Date:** 2026-07-06
- **Scope:** language

## Decision
A `use <alias>.<module>` path that resolves to a declared path dependency
([spec §6.2](../spec/06-modules-packages.md)) makes that dependency's
**top-level items reachable and executable**, not merely graph-validated. A
package's **public surface** is every top-level `fn`, `struct`, and `enum`
(with its variants and fields) declared at module scope; Keel Core has no
visibility modifier, so there is nothing to hide and no `pub` keyword is
introduced. A reference `<alias>.<name>` resolves to the top-level `<name>` of
the addressed dependency module; a `<name>` absent from that module is the
ordinary unresolved-name error, and an alias absent from `[dependencies]`
remains `K1105`.

The backend compiles the entire acyclic package graph into a **single
`package main` translation unit**, mangling every non-root package's top-level
symbols with a deterministic prefix derived from that package's **manifest
name** (not the importer's alias, so a symbol's emitted name is identical
regardless of who imports it or under what alias). This preserves the single
static binary, the hermetic/reproducible-build contract
([KDR-0105](0105-hermetic-reproducible-builds.md)), and the `FROM scratch`
image story ([KDR-0107](0107-oci-image-build.md)) with no per-package Go module
plumbing. The mangling scheme is part of the reproducible-build output
contract (root [`AGENTS.md`](../../AGENTS.md) hard rule 7).

## Context
Through M8 the workspace loader parses manifests, builds the path-dependency
graph, validates `use` paths, and enforces capabilities transitively — but
**does not compile dependency symbols into the root program**. Executable
compilation is single-source-module
(`docs/feature-status.md` Modules/Packages rows; `compiler/keelc-driver`). Case
`812-path-dependency` proves an alias *resolves*; no case proves a dependency
function *runs*. The M6 `users-service` and M7 multi-package workspace are
therefore multi-package in manifest and capability terms only — the language
cannot yet call a function defined in another package.

This is the load-bearing hole under the "working language" claim: nine
milestones delivered LSP, reproducible OCI images, `keel gen`, arenas, and
editions on top of a language whose packages cannot call each other. Cross-
package linking is priority zero, and no milestone currently owns it.

Prior art: Go compiles each package separately and links by import path, paying
per-package `go.mod`/visibility machinery and export/import ceremony. Rust
mangles across crates into one artifact and exposes visibility with `pub`. Keel
already flattens to one `package main`, so the Go per-package model would add
plumbing the compiler's own shape does not need.

## Alternatives considered
- **Emit one Go package per Keel package under a synthetic module path.**
  Rejected: multiplies `go.mod`/import-path plumbing and import-ordering
  non-determinism risk for a backend that already emits a single
  `package main`; buys nothing linking does not, and complicates the hermetic
  build and OCI digest guarantees.
- **Introduce a `pub`/visibility modifier now** so packages can hide internals.
  Rejected: a language-surface addition absent from Keel Core (hard rule 3) and
  premature — export granularity is a corpus question, not an a-priori one. If
  evidence demands it, a later KDR supersedes this one's "all top-level items
  public" clause without disturbing the linking mechanism.
- **Textually include/inline dependency source into the root file before
  compilation.** Rejected: destroys name hygiene and would let a dependency's
  `std` uses masquerade as the root's, breaking the capability boundary
  ([KDR-0011](0011-package-capabilities.md), [KDR-0043](0043-implicit-package-capability-trust-anchor.md)).
- **Mangle by importer alias rather than manifest name.** Rejected: the same
  dependency imported under two aliases would emit two symbol sets, breaking
  identity and bloating output; alias is a local naming convenience, manifest
  name is the stable identity.

## Consequences
- Cross-package calls compile and run; the M6/M7 examples become genuinely
  multi-package. A new accept-case must prove `<alias>.<fn>()` from a path
  dependency executes and prints, and the `812` sibling reject-cases stay green.
- No `pub` keyword: every top-level item in a dependency is callable. Packages
  cannot hide internals until a visibility KDR says otherwise.
- The mangling scheme joins the reproducible-build output contract; changing it
  later is a breaking output change, not a free refactor.
- Capability enforcement is unaffected: symbols cross the boundary, authority
  does not — a dependency's `std` obligations remain bounded by its own
  manifest, checked transitively as today.
- Sequenced spec → tests → implementation (hard rule 1): spec §6 gains the
  public-surface and `<alias>.<name>` resolution rules first, conformance gains
  the linked-call accept-case, then the driver/backend implement linking.

## Reopening clause  *(required)*
Reopen the **compilation model** (single `package main` + name mangling) only
on corpus evidence that flattening produces symbol collisions the mangling
scheme cannot resolve, or build-time/output-size blowup at realistic scale
(≥ the KDR-0019 reference-corpus package count) that a per-Go-package model
measurably avoids. Reopen the **"all top-level items public"** rule only on
corpus evidence that ≥5% of packages need to hide a top-level symbol to prevent
a demonstrated misuse — at which point a visibility modifier is specified in its
own KDR. Advocacy, "language X does it," and aesthetic preference are never
sufficient.
