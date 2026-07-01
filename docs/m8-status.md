# M8 status — incremental compiler core + LSP

Non-normative planning note for M8. The milestone boundary and exit criterion
live in [`ROADMAP.md`](../ROADMAP.md) §M8. Compiler behavior remains defined by
the specs and [`tests/conformance/`](../tests/conformance/).

## Goal and status

M8 makes the existing compiler pipeline incrementally reusable, measures the
compile-time contract, then exposes the same queries through `keel lsp`.

**Decision slice started.** [`KDR-0106`](kdr/0106-query-engine.md) accepts
Salsa as the query engine and fixes the M8 input/query boundary. M7 is green at
221 passed, 0 failed, 4 intentionally gated Core rejections. There is no query
database, public performance corpus, CI benchmark, `keelc-lsp` crate, or
`keel lsp` subcommand.

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
3. **Query implementation PRs.** Introduce source/config inputs and wrap the
   existing parse, resolve/typecheck, and KIR stages as deterministic queries.
   Repoint `keel check` first; repoint build/run only after check output is
   byte-identical. Stage functions remain free of I/O and global state.
4. **Gate PR.** Enable the KDR-0019 CI budgets only after the reference baseline
   is checked in and reproducible on the named machine.

### M8b — LSP

1. **Decision PR.** Accept or supersede proposed KDR-0103. No LSP dependency or
   crate lands while its decision remains proposed.
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
- [KDR-0103](kdr/0103-lsp-server.md) — proposed LSP decision; must be accepted or
  superseded before implementation.
- [Spec chapter 16](spec/16-lsp.md) — protocol surface and lifecycle.
- [Compiler architecture](../compiler/ARCHITECTURE.md) — pipeline and query-core
  constraints.
- [Root agent rules](../AGENTS.md) — concern separation, dependency discipline,
  determinism, no panics, and executable definition of done.

## Validation snapshot

Planning only; no M8 implementation exists. Last completed gate:

```text
KEEL_MILESTONE=M7 scripts/preflight.sh
221 passed, 0 failed, 4 skipped
```
