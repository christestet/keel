# KDR-0106: Salsa query engine for keelc

- **Status:** accepted
- **Date:** 2026-07-01
- **Scope:** toolchain

## Decision

Keelc will use the Rust `salsa` crate as its query engine for M8's incremental
compiler core, starting with the current `0.27` release line. The exact patch
version is pinned by `Cargo.lock` when the implementation PR adds the
dependency; upgrades are ordinary dependency changes and must preserve the M8
diagnostic, formatter, generated-source, and performance fixtures.

The query boundary is source-driven and side-effect free:

- inputs: workspace root, package manifest data, source file identity, source
  text, milestone, and command configuration that affects diagnostics;
- tracked queries: lex/parse, name resolution plus typechecking, KIR lowering,
  package/capability analysis, generated Keel from `keel gen`, and backend text
  emission;
- driver-only effects: filesystem discovery, file reads, `go build`, process
  execution, temporary-directory creation, and terminal output formatting.

`keel check` is the first command routed through the database. `build`, `run`,
`test`, `fmt`, `audit`, and `gen` move onto the same database only after their
observable output is byte-identical to the direct pipeline. Query functions may
accumulate diagnostics, but they must not print, read files, spawn processes, or
depend on wall-clock time.

## Context

KDR-0019 requires a query-based compiler core and sets concrete budgets: cold
build under 10 seconds, incremental build under 1 second, and `keel check` under
300 ms on the public reference corpus. `compiler/ARCHITECTURE.md` has kept the
pipeline shaped around a salsa-style graph, but that architecture note does not
authorize a dependency by itself.

Salsa is the Rust ecosystem's purpose-built framework for on-demand,
incremental computation. It models stable inputs separately from derived query
functions, memoizes query results, and invalidates only affected dependents when
inputs change. That matches Keel's M8 needs: the CLI and future LSP must reuse
the same parse, resolve/typecheck, KIR, and emission work without introducing a
second compiler path.

Salsa also has real risk. Its own project describes it as a work in progress,
and the current crate brings a non-trivial transitive dependency set. Keel
accepts that cost because KDR-0019 makes incrementality a release gate, not an
optimization wish, and because building an equivalent engine would become its
own compiler project.

## Alternatives considered

- **Keep the direct pipeline and optimize hot paths.** Rejected: this may make
  one `keel check` fast, but it does not give LSP document changes stable
  reuse, dependency tracking, or invalidation semantics.
- **Build a Keel-specific query engine in-house.** Rejected: the first
  implementation would have to solve memoization, revision tracking,
  dependency recording, cancellation, cycle handling, durability, and
  correctness testing before any language work benefits from it.
- **Adopt rustc's internal query machinery.** Rejected: it is not a stable,
  standalone library, and coupling Keel to rustc internals would make compiler
  distribution and upgrades harder than using a crate intended for this role.
- **Use file-level caches without a dependency graph.** Rejected: coarse caches
  are simpler, but they force global invalidation for common edits and would
  make the M8 LSP budget depend on corpus shape rather than the actual affected
  query set.

## Consequences

- A future implementation PR may add `salsa` to the compiler workspace without
  a separate dependency-justification KDR, but still may not mix that dependency
  addition with unrelated language, spec, or conformance changes.
- Compiler stages must keep accepting explicit inputs and returning structured
  outputs. Any stage that reads files, prints, consults global state, or hides
  process execution behind a query violates this KDR.
- Diagnostic order, formatter output, generated Keel, generated Go, and audit
  reports remain part of the public behavior. Moving a command onto the query
  database is only complete when byte-identical fixtures or conformance runs
  prove no observable drift.
- Salsa cancellation and cycle behavior are implementation details. They may be
  used to service LSP requests, but they must not surface as uncaught panics or
  replace Keel diagnostics for malformed user input.
- The dependency is deliberately limited to the compiler/tooling. It does not
  affect Keel language semantics, the standard library surface, package
  capabilities, or generated user code.

## Reopening clause

Reopen this decision if one of the following is demonstrated:

- the M8 reference corpus cannot meet the KDR-0019 budgets with Salsa after
  stage-level profiling identifies Salsa overhead, rather than Keel stage work,
  as the blocker;
- a Salsa release required for security or compiler compatibility makes Keel's
  query graph nondeterministic, unable to preserve byte-identical output, or
  incompatible with the no-panics-on-user-input rule;
- a maintained Rust query engine with materially lower dependency cost and
  equivalent incremental invalidation, diagnostics accumulation, and LSP reuse
  support is available and proven on the reference corpus.
