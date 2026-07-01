# M8 status — incremental compiler core + LSP

Non-normative planning note for M8. The milestone boundary and exit criterion
live in [`ROADMAP.md`](../ROADMAP.md) §M8. Compiler behavior remains defined by
the specs and [`tests/conformance/`](../tests/conformance/).
The first public preview release gate is tracked in
[`0.1.0 release readiness`](0.1-release-readiness.md).

## Goal and status

M8 makes the existing compiler pipeline incrementally reusable, measures the
compile-time contract, then exposes the same queries through `keel lsp`.

**Implementation slice started.** [`KDR-0106`](kdr/0106-query-engine.md)
accepts Salsa as the query engine and fixes the M8 input/query boundary. M7 is
green at 221 passed, 0 failed, 4 intentionally gated Core rejections. `keel
check`, `keel run`, `keel test`, and `keel build` now route parse, resolve,
typecheck, KIR lowering, diagnostics, and Go emission through the `keelc-query`
Salsa database (its own crate now, not driver-internal — see below).
[`KDR-0103`](kdr/0103-lsp-server.md) now accepts the M8 LSP server boundary and
synchronous protocol stack.
[`tests/lsp`](../tests/lsp/README.md) transcript fixtures cover every M8 base
capability: initialization, diagnostics, UTF-16/CRLF position mapping,
shutdown, JSON-RPC errors, go-to-definition, hover, completion, document
symbols, incremental `didChange` re-checks, and multi-line position mapping.

**`keelc-lsp` and `keel lsp` now exist** and pass all ten fixtures byte-for-
byte (`compiler/keelc-lsp/tests/transcripts.rs` replays every
`tests/lsp/m8-base/*.json` fixture through the real dispatch loop — real
`Content-Length` framing in, real framed JSON-RPC out — as part of
`cargo test --workspace`). The query engine moved out of `keelc-driver` into a
new `keelc-query` crate so `keelc-lsp` can depend on it without a cycle
(`keelc-driver` depends on `keelc-lsp` for the `lsp` subcommand). Scope note:
definition/hover/completion/documentSymbol resolve module-level `fn`/`struct`
declarations and a small built-in table by name — there is no local-scope
(parameter/`let`-binding) resolution yet, because `keelc-resolve`'s
`ResolveOutput` carries diagnostics only, no name/definition index. That is
sufficient for the ten base-capability fixtures but is a known gap for a real
editor session; extending it needs a resolver-side name index, which is
`keelc-resolve` work, not `keelc-lsp` work.

**M8a performance gate: captured, and currently failing all three budgets.**
The `m8-benchmark` job in `.github/workflows/ci.yml` runs
`scripts/m8-benchmark.sh --mode full` on every compiler-touching PR, on the
standard GitHub-hosted `ubuntu-latest` runner — now the documented reference
machine (see
[`tests/performance/m8-reference/reference-machine.md`](../tests/performance/m8-reference/reference-machine.md)),
not a contributor's local hardware. That job's own [workflow run
28533408054](https://github.com/christestet/keel/actions/runs/28533408054)
(commit `4283835`) produced the first real baseline, now checked into
`baseline.tsv`:

```text
metric                   elapsed_ms  budget_ms  status
keel_check               794         300        over-budget
keel_build_cold          18770       10000      over-budget
keel_build_incremental   1701        1000       over-budget
```

This is not a noisy-sandbox artifact — it is the actual reference machine.
**All three KDR-0019 budgets are currently unmet**, so `--enforce` stays off
in CI: turning it on today would fail every future compiler PR on a
pre-existing gap instead of catching a new regression. `baseline.tsv` still
records these numbers so the 5% regression comparison catches further
slowdowns from this point. Closing the budget gap is real compiler
performance work (see the incrementality note below for `keel_check`'s and
`keel_build_incremental`'s likely largest lever; `keel_build_cold`'s 18.8 s
against a 10 s budget needs its own investigation — profiling hasn't started).

**Known limitation: `keel_build_incremental` does not measure real
incrementality yet.** `keelc-query::SourceFile` has one `text: String` field
for the whole file, so any edit invalidates the entire
parse→resolve→typecheck→lower→emit chain — Salsa only memoizes exact repeats
today, not per-declaration reuse. The benchmark script also runs `keel build`
twice as separate CLI processes, each starting a fresh database, so today's
"incremental" number is really two independent cold builds. Real per-
declaration incrementality needs finer-grained query inputs (splitting the
monolithic `SourceFile` text), which is a compiler-architecture change, not a
benchmark-script fix — tracked as a `--known-gap keel_build_incremental` flag
on the CI job rather than silently claimed as met. `keel lsp` reuses the same
coarse-grained input today too (a fresh `SourceFile` per request), so it does
not yet demonstrate real incremental reuse either, despite holding one
long-lived `QueryDatabase` per session.

## Ordered slices

### M8a — query core

1. **Decision PR.** Done in [`KDR-0106`](kdr/0106-query-engine.md): accept a
   toolchain KDR choosing the query engine and fixing
   its input/query boundaries. This is the dependency-justification PR required
   by the root harness; KDR-0019 mandates incrementality but does not authorize
   a particular crate version or integration surface.
2. **Performance-fixture PR.** Done: [`tests/performance/m8-reference/README.md`](../tests/performance/m8-reference/README.md)
   defines the public reference corpus, reference-machine description (the
   `m8-benchmark` CI job's own `ubuntu-latest` runner), benchmark command, and
   5% regression comparison; the `m8-benchmark` job runs it on every
   compiler-touching PR; `baseline.tsv` carries the first real captured
   numbers (see "Goal and status" above). Still open: those numbers exceed
   budget, so `--enforce` stays off — see slice 4.
3. **Query implementation PRs.** Started: `keel check`, `run`, `test`, and
   `build` use a Salsa `SourceFile` input and deterministic parse, resolve,
   typecheck, KIR-lowering, Go-emission, and diagnostic queries. Stage
   functions remain free of I/O and global state; filesystem/process effects
   remain in the driver.
4. **Gate PR.** Not started — and not just a flag flip. The captured baseline
   shows `keel_check` (794 ms), `keel_build_cold` (18.8 s), and
   `keel_build_incremental` (1.7 s) all over their KDR-0019 budgets on the
   reference machine itself. Enabling `--enforce` requires closing that gap
   first (compiler performance work — profiling `keel_check`'s hot path and
   `keel build`'s Go-toolchain-invocation cost, plus the query-granularity
   work noted above for the incremental number), not just recording numbers.
   `keel_build_incremental` stays `--known-gap` even after that, until the
   query core gets per-declaration granularity.

### M8b — LSP

1. **Decision PR.** Done in [`KDR-0103`](kdr/0103-lsp-server.md): accept the
   M8 base LSP capability set and the `lsp-server`/`lsp-types` protocol stack.
2. **Spec PR.** Done in [`docs/spec/16-lsp.md`](spec/16-lsp.md): chapter 16 now
   names the M8 base capability set explicitly and marks references,
   formatting, code actions, workspace symbols, rename, and inlay hints as
   deferred.
3. **Protocol-fixture PR.** Done in
   [`tests/lsp/m8-base`](../tests/lsp/m8-base): deterministic JSON-RPC
   transcripts cover initialize, open diagnostics, UTF-16/CRLF positions,
   shutdown, malformed JSON, unsupported methods, go-to-definition, hover,
   completion, document symbols, incremental `didChange` re-checks, and
   multi-line position mapping — every M8 base capability now has a golden
   transcript.
4. **Implementation PRs.** Done for the base capability set: the `keelc-lsp`
   crate and `keel lsp` subcommand exist, backed only by the `keelc-query`
   database, and advertise exactly the five M8 base capabilities (definition,
   completion, hover, document symbols, incremental sync) plus diagnostics
   publishing. Remaining implementation work is deepening symbol resolution
   (see the local-scope gap noted above), not adding new advertised
   capabilities.

References, formatting, code actions, workspace symbols, rename, and inlay hints
are excluded from M8 even though chapter 16 lists them as later extensions.

## Exit gate

M8 exits only when all of the following hold:

- the full M7 conformance gate remains 221/0/4 with byte-identical diagnostics,
  formatter output, generated Keel, and generated Go;
- cold build is <10s, incremental build <1s, and `keel check` <300ms on the
  published reference corpus/machine, with the >5% CI regression gate active;
- all golden transcripts pass for every advertised capability;
- malformed JSON-RPC and malformed Keel input produce errors/diagnostics and do
  not terminate the server;
- `scripts/preflight.sh` is green and its summary is recorded here.

For a 0.1.0 developer-preview release, M8a's query and performance gate is a
hard blocker. M8b's LSP surface may either ship fully transcript-backed or be
left out of the release; do not advertise partial semantic LSP capabilities.

## Dependency chain

- [KDR-0019](kdr/0019-compile-time-contract.md) — budgets and query-core mandate.
- [KDR-0106](kdr/0106-query-engine.md) — accepted Salsa query engine and query
  boundaries.
- [KDR-0103](kdr/0103-lsp-server.md) — accepted M8 LSP decision, capability
  boundary, and protocol dependency stack.
- [Spec chapter 16](spec/16-lsp.md) — protocol surface and lifecycle.
- [Compiler architecture](../compiler/ARCHITECTURE.md) — pipeline, crate
  layout (`keelc-query`, `keelc-lsp`), and query-core constraints.
- [Root agent rules](../AGENTS.md) — concern separation, dependency discipline,
  determinism, no panics, and executable definition of done.

## Validation snapshot

Current implementation snapshot:

```text
scripts/preflight.sh
lsp fixtures: ok (10 transcript(s))
91 passed, 0 failed, 134 skipped
(includes `cargo test --workspace`, which runs
compiler/keelc-lsp/tests/transcripts.rs replaying all 10 tests/lsp/m8-base
fixtures against the real keel-lsp dispatch loop)

KEEL_MILESTONE=M7 scripts/preflight.sh
221 passed, 0 failed, 4 skipped

m8-benchmark CI job, workflow run 28533408054, commit 4283835, ubuntu-latest
(2 vCPU / 7.8 GiB / AMD EPYC 7763), scripts/m8-benchmark.sh --mode full --known-gap keel_build_incremental
keel_check               794    300    0    over-budget
keel_build_cold          18770  10000  0    over-budget
keel_build_incremental   1701   1000   0    over-budget
(--enforce not yet passed to this CI invocation — see "Goal and status" above
for why: all three metrics are over budget on the reference machine itself,
so enforcing today's KDR-0019 budgets would block every future compiler PR,
not catch a new regression)
```
