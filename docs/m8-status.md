# M8 status — incremental compiler core + LSP

Non-normative planning note for M8. The milestone boundary and exit criterion
live in [`ROADMAP.md`](../ROADMAP.md) §M8. Compiler behavior remains defined by
the specs and [`tests/conformance/`](../tests/conformance/).

## Goal and status

M8 makes the existing compiler pipeline incrementally reusable, measures the
compile-time contract, then exposes the same queries through `keel lsp`.

**Implementation slice started.** [`KDR-0106`](kdr/0106-query-engine.md)
accepts Salsa as the query engine and fixes the M8 input/query boundary. M7 is
green at 221 passed, 0 failed, 4 intentionally gated Core rejections. `keel
check`, `keel run`, `keel test`, and `keel build` now route parse, resolve,
typecheck, KIR lowering, diagnostics, and Go emission through a driver-internal
Salsa database. [`KDR-0103`](kdr/0103-lsp-server.md) now accepts the M8 LSP
server boundary and synchronous protocol stack. There is no public performance
baseline, CI benchmark, `keelc-lsp` crate, `keel lsp` subcommand, or LSP
transcript fixture.

## Ordered slices

### M8a — query core

1. **Decision PR.** Done in [`KDR-0106`](kdr/0106-query-engine.md): accept a
   toolchain KDR choosing the query engine and fixing
   its input/query boundaries. This is the dependency-justification PR required
   by the root harness; KDR-0019 mandates incrementality but does not authorize
   a particular crate version or integration surface.
2. **Performance-fixture PR.** Started by
   [`tests/performance/m8-reference/README.md`](../tests/performance/m8-reference/README.md):
   add the public reference corpus, reference-machine description, benchmark
   command, and 5% regression comparison. Keep benchmark fixtures separate from
   compiler implementation. Baselines are still zero and the gate is not wired
   into CI.
3. **Query implementation PRs.** Started: `keel check`, `run`, `test`, and
   `build` use a Salsa `SourceFile` input and deterministic parse, resolve,
   typecheck, KIR-lowering, Go-emission, and diagnostic queries. Stage
   functions remain free of I/O and global state; filesystem/process effects
   remain in the driver.
4. **Gate PR.** Enable the KDR-0019 CI budgets only after the reference baseline
   is checked in and reproducible on the named machine.

### M8b — LSP

1. **Decision PR.** Done in [`KDR-0103`](kdr/0103-lsp-server.md): accept the
   M8 base LSP capability set and the `lsp-server`/`lsp-types` protocol stack.
2. **Spec PR.** Chapter 16 predates the numbered M8 roadmap and labels optional
   capabilities `M8+`/`M9+`. Replace those relative labels with an explicit base
   and future capability split; otherwise assigning M8 would accidentally pull
   references, formatting, code actions, and workspace symbols into the exit
   gate.
3. **Protocol-fixture PR.** Add deterministic JSON-RPC transcripts for
   initialize, incremental open/change, diagnostics, definition, hover,
   completion, document symbols, shutdown, and malformed requests. Positions
   include ASCII, non-BMP Unicode, CRLF, and multi-line cases to lock 0-based
   UTF-16 conversion.
4. **Implementation PRs.** Add the `keelc-lsp` crate and `keel lsp`, backed only
   by the M8a query database. Advertise exactly the implemented base capability
   set from spec chapter 16.

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

## Dependency chain

- [KDR-0019](kdr/0019-compile-time-contract.md) — budgets and query-core mandate.
- [KDR-0106](kdr/0106-query-engine.md) — accepted Salsa query engine and query
  boundaries.
- [KDR-0103](kdr/0103-lsp-server.md) — accepted M8 LSP decision, capability
  boundary, and protocol dependency stack.
- [Spec chapter 16](spec/16-lsp.md) — protocol surface and lifecycle.
- [Compiler architecture](../compiler/ARCHITECTURE.md) — pipeline and query-core
  constraints.
- [Root agent rules](../AGENTS.md) — concern separation, dependency discipline,
  determinism, no panics, and executable definition of done.

## Validation snapshot

Current implementation snapshot:

```text
scripts/preflight.sh
91 passed, 0 failed, 134 skipped

KEEL_MILESTONE=M7 scripts/preflight.sh
221 passed, 0 failed, 4 skipped

M8_REFERENCE_HANDLERS=3 scripts/m8-benchmark.sh --mode check --work-dir target/m8-reference-smoke --metrics target/m8-reference-smoke.tsv
keel_check	9	300	0	ok
```
