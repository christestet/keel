# M7 packages & capabilities — implementation wiki note

Non-normative. Tracks the package-manifest + capability slice — differentiator 1
of 6 — within M7; the milestone-wide live note is [`m7-status.md`](m7-status.md).
The governing texts are [`ROADMAP.md`](../ROADMAP.md) §M7,
[`KDR-0011`](kdr/0011-package-capabilities.md),
[`KDR-0017`](kdr/0017-function-capabilities.md),
[`KDR-0007`](kdr/0007-no-build-scripts.md), and the two spec chapters below —
this note links them, it does not restate them.

## Status

**Done — spec PR (PR-A + PR-B):**

- [`docs/spec/06-modules-packages.md`](spec/06-modules-packages.md): package =
  directory rooted at `keel.toml`; closed manifest schema (`[package]`
  name/version/edition/capabilities, `[dependencies]` path deps); module resolution by
  first segment (`std` / self / dependency alias); acyclic dependency graph;
  diagnostics `K1101`–`K1108`.
- [`docs/spec/11-capabilities.md`](spec/11-capabilities.md): the six
  capabilities (`net fs exec env ffi unsafe-memory`); normative stdlib
  capability map; package-level + transitive enforcement; effective-set rollup;
  `keel audit` (deterministic output); diagnostics `K1110`–`K1112`. Manifest
  format is `keel.toml`; the slice was scoped wide (manifest + deps + audit) per
  the planning decision.
- [`docs/spec/00-spec-plan.md`](spec/00-spec-plan.md) rows 06 and 11 marked
  **specified (impl pending)**.

**Explicitly not done:**

- No compiler code. `keel.toml` is read by nothing yet; `use std.http`/`std.sql`
  remain ungated as in M6. The `K11xx` codes are **named in the spec but not yet
  registered** in `keelc-diag` — registration rides with the implementation PR
  (the chapter-09 precedent: spec names the band, impl registers it).
- Registry/network dependencies (path deps only).
- Function-level `use capabilities(...)` annotations ([`KDR-0017`](kdr/0017-function-capabilities.md)).

**Done — conformance PR (PR-T):** cases `810`–`817` and `820`–`826`, including
manifest fixtures and nested path-dependency packages. The runner already uses
each case directory as the compiler working directory, so no new package mode
or runner code was necessary.

## Dependency chain

- Decisions: [`KDR-0011`](kdr/0011-package-capabilities.md) (capability set +
  threat model), [`KDR-0007`](kdr/0007-no-build-scripts.md) (manifest is data),
  [`KDR-0020`](kdr/0020-ecosystem-bootstrap.md) (path-first), [`KDR-0017`](kdr/0017-function-capabilities.md)
  (deferred function-level), [`KDR-0042`](kdr/0042-sqlite-driver-modernc.md)
  (why `std.sql` ⇒ `net`+`fs`).
- Stdlib surface mapped: [`15-stdlib-core.md`](spec/15-stdlib-core.md) §15.25
  (`log`→stdout, no cap), §15.28 (`sql`), §15.31 (`config`→env).
- Harness: root [`AGENTS.md`](../AGENTS.md) hard rule 1 (spec→tests→impl, three
  PRs), hard rule 6 (no panics on malformed manifests), hard rule 7 (deterministic
  audit/rollup output); [`docs/spec/AGENTS.md`](spec/AGENTS.md) (codes registered
  at spec time, append-only).

## Milestone boundary

M6 exit was met (conformance green; `users-service` runs full CRUD on SQLite).
M7 opens with this slice. The roadmap allows the rest of M7 next — `keel audit`
implementation, `arena` blocks, `keel gen`, hermetic builds, edition machinery —
but **not** the post-M7 LSP server until the salsa query core exists.

## Validation snapshot

Conformance PR. Gate:

```sh
scripts/preflight.sh        # harness self-check + workspace build/test + conformance structure
```

Structure: **212 cases valid**. M6: **194 passed, 0 failed, 18 skipped** (the
15 new M7 cases plus three post-M4 Core rejections). The M7 gate is expected to
fail until PR-I implements these cases.

## Next work

Next PR, per hard rule 1:

1. **PR-I (implementation).** Manifest parser (TOML → typed manifest, every
   malformed input a `K11xx` diagnostic, never a panic), dependency-graph
   resolver (path deps, cycle detection), `std`-use capability check, transitive
   rollup, and `keel audit`. Register `K1101`–`K1108` and `K1110`–`K1112` in
   [`compiler/keelc-diag/src/registry.rs`](../compiler/keelc-diag/src/registry.rs).
   No new dependency for TOML parsing without a justifying PR (hard rule 5) — a
   minimal hand-rolled reader over the closed schema may be lazier than pulling a
   crate; decide at PR-I time.
