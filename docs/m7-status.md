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
- Foundations for the other five differentiators are now in place: spec
  chapters [`10-memory.md`](spec/10-memory.md) (arena) and
  [`14-editions.md`](spec/14-editions.md) landed; new KDRs
  [`0104`](kdr/0104-keel-gen-codegen-surface.md) (`keel gen`) and
  [`0105`](kdr/0105-hermetic-reproducible-builds.md) (hermetic builds) record
  the two previously-undecided differentiators (proposed). `keel audit` needs no
  new decision — it is part of [`KDR-0011`](kdr/0011-package-capabilities.md),
  specified in [`11-capabilities.md`](spec/11-capabilities.md) §11.5.
- **No compiler code for any differentiator yet.** Specs/KDRs are the contracts;
  tests and implementation follow per hard rule 1.
- Conformance unchanged from M6 — all spec-only so far; cases land with the
  test PRs.

## The exit gate — OPEN

ROADMAP M7 **exit** requires all six differentiators demonstrable through the
packaged [`examples/users-service/`](../examples/users-service/) workspace, each
locked by conformance. The example is aspirational by design (as with M6); the
compiler grows to meet it.

| # | Differentiator | Demonstrand | State |
|---|---|---|---|
| 1 | Manifests + capabilities | per-package `keel.toml`; transitive enforcement; `K1110` reject | spec landed, impl pending |
| 2 | `keel audit` | deterministic effective-capability report (spec §11.5) | spec landed (§11.5), impl pending |
| 3 | `arena` | `arena { }` scratch region compiles + runs safely | spec landed (ch10), impl pending |
| 4 | `keel gen` | service types from protobuf/OpenAPI; round-trips `keel fmt` | KDR-0104 landed, spec + impl pending |
| 5 | Hermetic builds | two clean builds byte-identical, no host/net leakage | KDR-0105 landed, spec + impl pending |
| 6 | Editions | manifest `edition` honored; unknown edition diagnosed | spec landed (ch14), impl pending |

## Dependency chain

- Decisions: [`KDR-0011`](kdr/0011-package-capabilities.md) (capabilities),
  [`KDR-0017`](kdr/0017-function-capabilities.md) (deferred function-level),
  [`KDR-0007`](kdr/0007-no-build-scripts.md) (declarative manifest),
  [`KDR-0012`](kdr/0012-gc-plus-scoped-arenas.md) /
  [`KDR-0016`](kdr/0016-scope-implicit-arenas.md) (arena),
  [`KDR-0001`](kdr/0001-editions.md) (editions),
  [`KDR-0020`](kdr/0020-ecosystem-bootstrap.md) (path-first packaging).
- Specs: chapters [`06`](spec/06-modules-packages.md) /
  [`11`](spec/11-capabilities.md) / [`10`](spec/10-memory.md) (arena) /
  [`14`](spec/14-editions.md) (editions) landed; chapter 12 (FFI) to be authored.
  `keel gen` ([`KDR-0104`](kdr/0104-keel-gen-codegen-surface.md)) and hermetic
  builds ([`KDR-0105`](kdr/0105-hermetic-reproducible-builds.md)) now have
  decisions; their spec chapters are not yet authored.
- Harness: root [`AGENTS.md`](../AGENTS.md) hard rule 1 (spec→tests→impl),
  rule 5 (no new deps without a justifying PR — relevant to TOML, protobuf,
  OpenAPI), rule 6 (no panics on manifests/schemas), rule 7 (deterministic
  audit + hermetic output).

## Step sequence

Each differentiator is its own KDR(s) → spec chapter → conformance → compiler
chain. KDRs and spec chapters are in place for capabilities, audit, arena, and
editions; `keel gen` / hermetic builds have KDRs, specs pending. **No compiler
code is started for any of them.**

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
3. **Editions** — spec [`ch14`](spec/14-editions.md) landed. Impl: edition gate
   in the compiler, `K1401` unknown-edition, `K1402` preview gating; `K1403`
   registered, untriggered until an edition past 1. The `keel.toml` slot exists
   (ch06).
4. **`arena`** — spec [`ch10`](spec/10-memory.md) landed. Impl: `arena { }`
   lowering onto the Go runtime within KDR-0012's safety guarantees, escape
   analysis emitting `K1001`, scope/arena interaction (spec
   ch09 §9.7 already names the boundary).
5. **`keel gen`** — decision recorded
   ([`KDR-0104`](kdr/0104-keel-gen-codegen-surface.md)). Next: a spec chapter for
   the proto/OpenAPI → Keel mapping, then a `keel gen` command that emits
   deterministic, `keel fmt`-clean, capability-declared, stdlib-only source.
6. **Hermetic builds** — decision recorded
   ([`KDR-0105`](kdr/0105-hermetic-reproducible-builds.md)). Next: a CI check
   asserting two clean builds are byte-identical, and enforcement of the
   no-network / no-host-dependence build constraints.

## Validation snapshot

Spec + roadmap slice. No new conformance cases yet. Gate:

```sh
scripts/preflight.sh        # harness self-check + workspace build/test + conformance structure
```

Last run: **91 passed, 0 failed** at the default milestone; M6 full run is
194 / 0 / 3. Nothing in the suite moves until the first test PR (PR-T).
