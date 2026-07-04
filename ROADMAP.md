# Keel Implementation Roadmap

Milestones are in strict delivery order. Each milestone has a binary exit
criterion. Do not start milestone N+1 work while N's exit criterion fails.

## Delivery cycle

Every milestone is delivered as the smallest independently reviewable slices.
Each slice follows the repository's concern-separated sequence:

1. accept or supersede the governing KDR when a decision is still open;
2. land normative spec text;
3. land failing conformance cases or protocol/performance fixtures;
4. implement until those fixtures pass without regressing earlier milestones;
5. record the exact `scripts/preflight.sh` result in the milestone status note.

KDR, spec, tests, and implementation remain separate PRs. A milestone opens
only after the preceding exit gate is green; a status note may scope later work
but does not authorize implementation early.

## M0 — Freeze Keel Core and its conformance suite  *(no compiler code)*

Define the minimal language subset in [`docs/spec/keel-core.md`](docs/spec/keel-core.md) and express it as
executable conformance tests in [`tests/conformance/`](tests/conformance/). The subset: functions,
`let`/`mut`, primitive types, `String`, `struct`, `enum` (with payloads),
`match` (exhaustive), `Option`/`Result`, `?`, `catch`, blocks/expressions,
`List<T>`/`Map<K,V>` (built-in, not user generics yet), string interpolation,
modules/`use`, `print` for test output. Explicitly deferred: user-defined
generics, interfaces, `scope`/`spawn`, `arena`, capabilities, FFI, stdlib.

**Exit:** ≥ 60 conformance cases covering both accept (run + expected stdout)
and reject (expected compile error code) behavior, reviewed and frozen.

## M1 — Frontend: lexer, parser, AST, diagnostics

Implementation language: **Rust** (see [`compiler/ARCHITECTURE.md`](compiler/ARCHITECTURE.md), [KDR-0101](docs/kdr/0101-compiler-in-rust.md)).
Hand-written recursive-descent parser (good errors beat generated parsers).
Diagnostics carry stable error codes (`K####`) from day one — conformance
reject-tests match on codes, not message text.

**Exit:** every M0 case lexes+parses or fails with the right `K####` syntax code.

## M2 — Semantic analysis: resolver + typechecker

Name resolution, type checking (explicit signatures, local inference only),
exhaustiveness checking for `match`, no-implicit-zero struct construction,
`Option`/`Result` semantics, `?`/`catch` typing with union error types.

**Exit:** all M0 reject-cases produce their exact error codes; accept-cases typecheck.

## M3 — First backend: compile to Go  *([KDR-0102](docs/kdr/0102-go-backend-first.md))*

Lower the typed AST to a Keel IR, emit Go source, drive `go build` internally.
This buys a production-grade concurrent GC, scheduler, cross-compilation and
static binaries for free, making Keel programs *runnable* years earlier. The Go
backend is scaffolding, not destiny: a native backend replaces it later, and the
conformance suite is what guarantees identical behavior when it does.

**Exit:** `keelc run` passes 100% of M0 accept-cases. `examples/hello.keel` works.

## M4 — Toolchain skeleton: `keel` CLI, `fmt`, `test`

Single binary UX: `keel build|run|fmt|test`. `keel fmt` is canonical from the
first release — formatting freezes *now*, while the corpus is small.
`keel test` discovers `test "name" { }` blocks; `assert` with structural diffs.

**Exit:** `keel test` runs a Keel-language test file; `keel fmt` is idempotent on every Core file in the repo (post-Core examples, e.g. `examples/users-service/`, remain out of scope until their features land).

## M5 — Language completion wave 1

Interfaces (≤5 methods), interface-constrained user generics, and
`scope`/`spawn` structured concurrency on the Go runtime.

**Exit reached:** cases 212–233 and 710–723 pass at M5; formatter and backend
support are complete. See [`docs/milestone-status.md`](docs/milestone-status.md)
§M5.

## M6 — Stdlib slice + the demo service

`std.http`, `std.json`, `std.sql` (SQLite first, then Postgres), `std.log`,
`std.config`. **Exit:** `examples/users-service/main.keel` from the design
discussion compiles, runs, and passes its test file.

**Exit reached:** the users service runs full CRUD on SQLite; cases 804 and 806
lock the database path. See [`docs/milestone-status.md`](docs/milestone-status.md) §M6.

## M7 — The differentiators

Package manifests + capability enforcement, `keel audit`, `arena` blocks,
`keel gen` for protobuf/OpenAPI, hermetic builds, and edition machinery in the
compiler (must exist before 1.0 even though edition 2 is years away). Specs:
[`06-modules-packages.md`](docs/spec/06-modules-packages.md) and
[`11-capabilities.md`](docs/spec/11-capabilities.md) are **specified** (impl
pending); chapters 10 (arena/memory), 12 (FFI), 14 (editions) follow under the
same spec → tests → impl discipline. See [`docs/milestone-status.md`](docs/milestone-status.md) §M7.

**Exit (all six must hold — every differentiator demonstrable).** As with M6,
the demo is aspirational and the compiler grows to meet it: the packaged
`examples/users-service/` builds and runs as a multi-package workspace whose
gate exercises each differentiator end to end, every behavior locked by
conformance:

1. **Manifests + capabilities.** Each package carries a `keel.toml` declaring
   its `edition`, `capabilities`, and path `[dependencies]`; the service
   declares `net`+`fs`, a local helper package declares none; declarations are
   enforced transitively, and a variant omitting a required capability fails
   with `K1110`.
2. **`keel audit`.** Produces the deterministic, byte-identical effective-
   capability report for the workspace dependency graph
   ([`11-capabilities.md`](docs/spec/11-capabilities.md) §11.5).
3. **`arena`.** The service uses an `arena { }` scratch region that compiles and
   runs within Keel's safety guarantees ([KDR-0012](docs/kdr/0012-gc-plus-scoped-arenas.md), spec chapter 10).
4. **`keel gen`.** Request/response types the service consumes are generated by
   `keel gen` from a protobuf or OpenAPI schema, and the generated Keel
   round-trips `keel fmt`.
5. **Hermetic builds.** `keel build` is reproducible: two clean builds of the
   workspace produce byte-identical output with no host/network leakage
   (determinism, root [`AGENTS.md`](AGENTS.md) hard rule 7).
6. **Editions.** The manifest's declared `edition` is honored by the compiler;
   an unknown edition is a diagnostic, and the edition gate exists in the
   compiler though edition 2 is years away ([KDR-0001](docs/kdr/0001-editions.md)).

**Exit reached:** 221 passed, 0 failed, 4 intentionally gated Core rejections.
See [`docs/milestone-status.md`](docs/milestone-status.md) §M7.

## M8 — Incremental compiler core + LSP

M8 has two ordered slices. M8a makes compiler work reusable and measurable;
M8b exposes that work through LSP. See
[`docs/milestone-status.md`](docs/milestone-status.md) §M8.

The published `0.1.x` developer preview is not identical to M8 exit. Its
release, documentation, installation, and known-limitation scope is in
[`docs/compatibility.md`](docs/compatibility.md).

### M8a — Query core and performance gate

- Accept a toolchain KDR for the query-engine dependency and input/query
  boundaries; dependency approval is not inferred from the architecture prose.
- Route `keel check` and the existing driver through one in-process query
  database without changing diagnostics, formatting, generated Go, or runtime
  behavior.
- Add the public reference corpus and CI benchmark required by
  [KDR-0019](docs/kdr/0019-compile-time-contract.md): cold build <10s,
  incremental build <1s, and `keel check` <300ms on the reference machine,
  with regressions over 5% blocking release.

### M8b — LSP

- Accept or supersede proposed [KDR-0103](docs/kdr/0103-lsp-server.md) before
  adding its dependencies or crate.
- Rebase chapter 16's old `M7+`/`M8+`/`M9+` labels into an explicit base and
  future capability split in a spec-only PR before adding transcript fixtures.
- Add `keel lsp` with the base chapter-16 surface only: incremental document
  sync, diagnostics, definition, hover, completion, and document symbols.
- Map byte spans to 0-based UTF-16 LSP positions and lock behavior with
  deterministic JSON-RPC transcript fixtures.

**Exit:** all M7 conformance remains byte-identical; the three KDR-0019 budgets
pass in CI; a golden initialize/open/change/query/shutdown transcript passes for
each advertised capability; malformed requests and malformed Keel source never
crash the server.

## M9 — Reproducible OCI images

Extend the chapter-18 hermetic build contract from the binary to its minimal
deployment artifact. First land a toolchain KDR and normative spec chapter 19;
then add daemonless `keel build --image`, producing a `FROM scratch`-equivalent
OCI image from the already-built static binary.

**Exit:** a new image conformance mode builds the same workspace twice and
asserts byte-identical OCI output and digest, validates the OCI layout/config,
and verifies that no network, Docker daemon, timestamp, path, or host metadata
enters the artifact.

Explicitly out of scope: language-level health probes, Helm charts, operators,
UPX/strip policy, and new shutdown machinery. Existing HTTP, config, static
binary, and structured-concurrency behavior already covers those runtime needs.

## M10 — Ecosystem bridges

Complete the two accepted bootstrap paths from
[KDR-0020](docs/kdr/0020-ecosystem-bootstrap.md) as separate slices:

1. Author spec chapter 12, then implement the smallest specified C ABI
   `extern` surface end to end. `extern` requires the package's `ffi`
   capability, each crossing appears in `keel audit`, and malformed declarations
   diagnose rather than panic.
2. Extend chapter 17 and `keel gen` from the proto3 data subset to the OpenAPI
   client/server output required by
   [KDR-0104](docs/kdr/0104-keel-gen-codegen-surface.md). Output remains
   deterministic, `keel fmt`-clean, stdlib-only, and capability-declared.

**Exit:** conformance calls a reference C library through an audited `extern`
boundary and generates, builds, and runs a typed client/server pair from a
fixed OpenAPI fixture. Unsupported ABI/schema constructs fail with stable
diagnostics rather than being guessed.

## M11 — Native backend and 1.0 gate

[KDR-0102](docs/kdr/0102-go-backend-first.md) requires the Go-emitting backend
to be replaced before 1.0. M11 begins with a KDR choosing the native backend and
runtime strategy; no backend implementation starts before that decision lands.
The Go backend remains available during equivalence work and the conformance
suite is the proof boundary.

**Exit:** every conformance case passes with byte-identical observable behavior
under both backends; the native backend satisfies KDR-0019 and chapter-18
reproducibility, emits static cross-compiled binaries, implements real arena
regions with complete escape checking, and the release toolchain no longer
requires Go. The native-backend KDR supersedes KDR-0102.

## Trigger-gated work (not scheduled)

- `K1402`, `--preview`, and case 843 wait for an approved preview feature.
- `keel fix` and `K1403` wait for the first concrete edition migration; they
  are mandatory parts of that edition's release gate, not standalone commands
  built against no migration.
- Function-level capabilities wait for KDR-0017 acceptance backed by corpus
  evidence that package-level reporting is insufficient.
- Additional arena analysis belongs to M11's real region backend; extending the
  current no-op Go lowering earlier would not improve runtime behavior.

## Performance contract (measured from M8 onward)

M8 creates the reference corpus and CI harness required by KDR-0019. From that
point, regressions >5% block merge
([vision.md §7](docs/vision.md#7-compile-time-as-a-contract)). See
[`compiler/ARCHITECTURE.md`](compiler/ARCHITECTURE.md) for current status.

## Validating the active milestone

`scripts/preflight.sh` is the executable definition of done. When validating a
specific milestone, set `KEEL_MILESTONE=M<N>` so the conformance runner uses the
same milestone gate as the roadmap item being claimed. Example for M3:

```sh
KEEL_MILESTONE=M3 scripts/preflight.sh
```

The runner accepts the same value as `--milestone M<N>`; see
[`tests/conformance/README.md`](tests/conformance/README.md).
