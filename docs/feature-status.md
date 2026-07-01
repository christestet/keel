# Feature status

This is the user-facing implementation snapshot. It summarizes implementation,
not future design intent. Normative behavior lives in the specification and
conformance suite; detailed work history lives in
[milestone status](milestone-status.md). The first-release gate is tracked in
[`0.1.0 release readiness`](0.1-release-readiness.md).

Status meanings:

- **Implemented** — compiler/backend behavior has conformance coverage.
- **Partial** — a useful slice exists, with named missing behavior.
- **Specified** — normative design exists but implementation is absent or
  insufficient.
- **Planned** — roadmap work; do not write production source against it.

Current M7 gate: **221 passed, 0 failed, 4 intentionally skipped**. The skipped
cases are earlier-milestone rejection traps for features that subsequently
landed. M8 implementation has started, but there is no 0.1.0 release yet.

## Language

| Feature | Status | Current boundary |
|---|---|---|
| Core functions, bindings, structs, enums, exhaustive match | Implemented | frozen Core plus later conformance additions |
| `Option`, `Result`, `?`, `catch`, union errors | Implemented | universal `Error` is opaque |
| Interfaces | Implemented | nominal, explicit impls, at most five methods |
| Constrained generics | Implemented | every type parameter has an interface bound; Go backend erases bounds |
| Structured concurrency | Implemented | `scope`/`spawn`, join, fail-fast, deadlines, cancellation checkpoints |
| Arenas | Partial | syntax and tail escape check; Go backend uses an ordinary block, not a region allocator |
| Modules | Partial | headers/imports parse; executable compilation remains single-source-module |
| Packages | Partial | local path graph/import validation; dependency source is not linked into the root module |
| Package capabilities | Partial | enforced for explicit manifests; implicit packages bypass enforcement |
| Editions | Partial | edition 1 selection and unknown-edition error only |
| C FFI / `extern` | Planned (M10) | rejected with `K0905` today |
| Function-level capabilities | Unscheduled | proposed KDR-0017, no syntax/implementation |
| Preview features and `keel fix` | Trigger-gated | no approved preview or edition migration exists |

## Standard library

| Surface | Status | Current boundary |
|---|---|---|
| `std.time` | Implemented | durations, cancellation-aware sleep, deadlines/checkpoints |
| `std.json` | Implemented | strict/tolerant parsing and deterministic writing for conformance-backed types |
| `std.http` | Partial | router/server, typed path/query params, response helpers; raw query/header methods lack backend coverage |
| `std.log` | Implemented base | one-string info/warn/error; structured fields are not stable |
| `std.sql` | Partial | bundled SQLite, positional parameters, mapping/collect; other drivers and `next()` are unavailable |
| `std.config` | Implemented | env-backed named structs, defaults, `Option`, and `Secret` |
| `Uuid`, `Timestamp`, `Email` | Implemented | closed compiler-known scalar set |
| Postgres driver | Not implemented | URL prefix is recognized but no Go driver is linked |
| OpenTelemetry/probes | Not implemented | vision goal only |

## Toolchain

| Tool | Status | Current boundary |
|---|---|---|
| `keel check` | Implemented | routed through the M8 Salsa query database for parse/resolve/typecheck |
| `keel run` | Implemented | query-backed KIR/Go emission, then temporary `go run` |
| `keel build` | Partial | query-backed KIR/Go emission plus reproducible flags; SQL may resolve Go modules over network |
| `keel test` | Implemented | query-backed Go test harness generation |
| `keel fmt` | Implemented | canonical stdout formatter; does not edit files |
| `keel audit` | Implemented slice | deterministic explicit-package capability report |
| `keel gen` | Partial | proto3 data subset only; OpenAPI/client/server generation is M10 |
| `keel lsp` | Partial | `keelc-lsp` crate + subcommand implemented; all ten base-capability protocol fixtures pass byte-for-byte; definition/hover/completion/documentSymbol resolve module-level `fn`/`struct` declarations only, not local scopes |
| `keel build --image` | Planned (M9) | no OCI image output |
| `keel lint` | Not implemented | waiver/lint design is not a command today |
| `keel fix` | Trigger-gated | requires a concrete edition migration |
| Package registry/publish | Not implemented | path dependencies only |
| Native backend | Planned (M11) | Go toolchain remains required for executable generation |

## Documentation/specification gaps

- Spec chapters 2, 3, 5, 12, and 13 are not authored as standalone full
  chapters; Core and conformance cover existing behavior where applicable.
- Chapter 16 has been rebased to an explicit M8 base/deferred split. All base
  capability protocol fixtures exist and pass against the real `keel lsp`
  server; local-scope (parameter/`let`-binding) symbol resolution is not yet
  implemented, since `keelc-resolve` has no name/definition index.
- Structured log arguments are accepted by current code but remain explicitly
  aspirational in chapter 15 and lack conformance coverage.
- The strong no-network hermetic-build prose conflicts with current SQL module
  resolution. Documentation treats the implementation as the present limit;
  the normative/implementation gap still needs resolution.
- 0.1.0 release readiness is blocked on nonzero M8 performance baselines and
  CI enforcement (`keel lsp` itself now ships the full base capability set,
  so [`docs/0.1-release-readiness.md`](0.1-release-readiness.md)'s LSP choice
  is "ship it," not "omit it") and on release/install/version reporting.

When a row changes, update this page in the same concern as the status change;
do not present roadmap intent as implemented behavior.
