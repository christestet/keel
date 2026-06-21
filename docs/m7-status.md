# M7 status — the differentiators

Single live note for M7. Non-normative. The governing language definition is
[`docs/spec/keel-core.md`](spec/keel-core.md); the executable spec is
[`tests/conformance/`](../tests/conformance/); milestone scope and the exit
criterion live in [`ROADMAP.md`](../ROADMAP.md) §M7. Decisions are made in
[`docs/kdr/`](kdr/) under the three-PR discipline in [`AGENTS.md`](../AGENTS.md)
(spec → tests → impl, each its own concern).

This note plans work; it does not authorize it. Every step below is still a
KDR → spec → conformance → compiler sequence.

## Goal

M7 turns "one service runs" (M6) into "build auditable, reproducible
multi-package apps." It is the set of features no incumbent bundles by default:
package-level capability enforcement, a one-command supply-chain audit, scoped
arenas, schema codegen, hermetic builds, and edition machinery.

## Status

- M6 exit reached (conformance **194 / 0 / 3**; `users-service` runs full CRUD
  on SQLite). No earlier gate regressed.
- M7 opened with the **package + capability spec slice**: chapters
  [`06-modules-packages.md`](spec/06-modules-packages.md) and
  [`11-capabilities.md`](spec/11-capabilities.md) are landed (specified, impl
  pending); the `keel.toml` `edition` slot is opened (ch06). See the slice note
  [`m7-packages-capabilities.md`](m7-packages-capabilities.md).
- Everything else (audit impl, arena, `keel gen`, hermetic builds, editions)
  is **not started**.
- Conformance unchanged from M6 — this slice is spec-only; cases land with the
  test PRs.

## The exit gate — OPEN

ROADMAP M7 **exit** requires all six differentiators demonstrable through the
packaged [`examples/users-service/`](../examples/users-service/) workspace, each
locked by conformance. The example is aspirational by design (as with M6); the
compiler grows to meet it.

| # | Differentiator | Demonstrand | State |
|---|---|---|---|
| 1 | Manifests + capabilities | per-package `keel.toml`; transitive enforcement; `K1110` reject | spec landed, impl pending |
| 2 | `keel audit` | deterministic effective-capability report (spec §11.5) | not started |
| 3 | `arena` | `arena { }` scratch region compiles + runs safely | not started |
| 4 | `keel gen` | service types from protobuf/OpenAPI; round-trips `keel fmt` | not started |
| 5 | Hermetic builds | two clean builds byte-identical, no host/net leakage | not started |
| 6 | Editions | manifest `edition` honored; unknown edition diagnosed | spec slot opened (ch06); semantics pending |

## Dependency chain

- Decisions: [`KDR-0011`](kdr/0011-package-capabilities.md) (capabilities),
  [`KDR-0017`](kdr/0017-function-capabilities.md) (deferred function-level),
  [`KDR-0007`](kdr/0007-no-build-scripts.md) (declarative manifest),
  [`KDR-0012`](kdr/0012-gc-plus-scoped-arenas.md) /
  [`KDR-0016`](kdr/0016-scope-implicit-arenas.md) (arena),
  [`KDR-0001`](kdr/0001-editions.md) (editions),
  [`KDR-0020`](kdr/0020-ecosystem-bootstrap.md) (path-first packaging).
- Specs: chapters [`06`](spec/06-modules-packages.md) /
  [`11`](spec/11-capabilities.md) (landed); chapters 10 (memory/arena),
  12 (FFI), 14 (editions) to be authored; `keel gen` and hermetic builds need
  their own spec chapters or KDRs (none yet — premature to invent).
- Harness: root [`AGENTS.md`](../AGENTS.md) hard rule 1 (spec→tests→impl),
  rule 5 (no new deps without a justifying PR — relevant to TOML, protobuf,
  OpenAPI), rule 6 (no panics on manifests/schemas), rule 7 (deterministic
  audit + hermetic output).

## Step sequence

Each differentiator is its own KDR(s) → spec chapter → conformance → compiler
chain; none is started in the compiler yet.

1. **Capabilities** (in progress). Spec done. Next: **PR-T** — cases `810`–`817`,
   `820`–`826`; the conformance runner needs a **package-aware mode** (a case
   carries a `keel.toml` and, for dep cases, sibling package dirs) before these
   can be expressed. Then **PR-I** — manifest parser (every malformed input a
   `K11xx` diagnostic, never a panic), path-dep resolver + cycle detection,
   `std`-use capability check, transitive rollup, registering `K1101`–`K1108`
   and `K1110`–`K1112`. Entry points: `compiler/conformance-runner`,
   `compiler/keelc-diag/src/registry.rs`, a new manifest crate.
2. **`keel audit`** — built on PR-I's rollup; a `keel audit` subcommand emitting
   the deterministic report. Conformance asserts byte-identical output.
3. **Editions** — chapter 14 + KDR-0001 expansion: edition value set, the
   compiler gate, unknown-edition diagnostic. The `keel.toml` slot already
   exists (ch06).
4. **`arena`** — chapter 10 (memory): `arena { }` syntax, lowering onto the Go
   runtime within KDR-0012's safety guarantees, scope/arena interaction (spec
   ch09 §9.7 already names the boundary).
5. **`keel gen`** — needs a KDR (protobuf/OpenAPI surface, dependency policy
   under hard rule 5) before any spec; generates Keel that must round-trip
   `keel fmt`.
6. **Hermetic builds** — likely a KDR (reproducibility contract, build sandbox)
   plus a CI check that two clean builds are byte-identical.

## Validation snapshot

Spec + roadmap slice. No new conformance cases yet. Gate:

```sh
scripts/preflight.sh        # harness self-check + workspace build/test + conformance structure
```

Last run: **91 passed, 0 failed** at the default milestone; M6 full run is
194 / 0 / 3. Nothing in the suite moves until the first test PR (PR-T).
