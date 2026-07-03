# KDR-0042: SQLite driver for the Go backend (`modernc.org/sqlite`)

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** toolchain

## Decision

When a compiled program uses `std.sql`, the Go backend emits a blank import of
**`modernc.org/sqlite`** and `keelc build`/`run` produce a Go module (a `go.mod`
plus `go mod tidy`) instead of a single-file build, so `database/sql` can resolve
the `"sqlite"` driver at runtime. This is a dependency of the *generated program*,
not of `keelc` itself. Programs that do not use `std.sql` keep the single-file,
stdlib-only build with no module and no network.

## Context

`std.sql` (KDR-0029) targets `database/sql`, which needs a registered driver. The
M6 exit program (`examples/users-service/main.keel`) must connect to SQLite, so a
driver is unavoidable. The runtime selects the driver by connection string and
already names it `"sqlite"`; only the import and the module-mode build were
missing.

## Alternatives considered

- **`mattn/go-sqlite3`.** The most common driver, but cgo: it needs a C
  toolchain, breaks `CGO_ENABLED=0`, and cross-compiles poorly — the opposite of
  the static-binary, hermetic-build goal (vision §) the Go backend exists to buy.
  Rejected.
- **Pure-Go `modernc.org/sqlite`.** No cgo, cross-compiles, single static binary,
  registers the `"sqlite"` driver name `database/sql` already expects. Larger
  binary (~14 MB) and a transitive dependency tree, accepted as the cost of no
  cgo. **Chosen.**
- **Bundle no driver; require the user to add one.** Rejected for M6: the demo
  service must run out of the box; driver choice is the toolchain's job.
- **Keep single-file builds, vendor the driver source.** Rejected: vendoring a
  pure-Go SQLite is large and duplicates what the module cache already does;
  module mode is the idiomatic Go path.

## Consequences

`std.sql` programs require network on first build (`go mod tidy` populates the
module cache) and link a larger binary. Builds stay deterministic (a pinned
module graph via `go.sum`). Non-sql programs are unaffected. Postgres/MySQL
drivers will follow the same blank-import pattern when those connection strings
are supported; the `keelSQLDriver` switch already anticipates them. Placeholder
translation (`$1` → SQLite `?1`) lives in the runtime, keeping the Keel-level
`$N` contract (KDR-0029) intact. Spec: §15 (`std.sql`). Conformance:
`804-sql-params-roundtrip`.

## Reopening clause  *(required)*

Reopen if a measured build-time or binary-size budget makes the pure-Go driver
untenable for a real corpus, or if a hermetic-build requirement forbids
`go mod tidy`'s network fetch (vendoring would then be reconsidered). A
preference for the more popular cgo driver is not, by itself, evidence.
