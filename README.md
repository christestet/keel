# ![Keel](keel-brand-kit/keel-logo-light-bg-256.png)

> A typed, compiled, garbage-collected language for backend services that should
> still be readable, reviewable, and deployable after five years of team churn.

**Status: compiler under active development.** Start with
[`docs/vision.md`](docs/vision.md) and [`ROADMAP.md`](ROADMAP.md).

## What Keel is

- One paradigm: procedural code over plain data, interfaces for polymorphism.
  No inheritance ([KDR-0003](docs/kdr/0003-no-inheritance.md)), no macros/reflection ([KDR-0004](docs/kdr/0004-no-macros.md)),
  no exceptions ([KDR-0005](docs/kdr/0005-no-exceptions.md)), no async/await ([KDR-0002](docs/kdr/0002-no-async-await.md)).
- Memory: concurrent low-latency GC + lexically scoped `arena` blocks ([KDR-0012](docs/kdr/0012-gc-plus-scoped-arenas.md)). No borrow checker.
- Safety: no null (`Option<T>`), no implicit zero values, exhaustive `match`,
  `Result` + `?` + `catch` for errors, union error types ([KDR-0005](docs/kdr/0005-no-exceptions.md)).
- Concurrency: structured only (`scope` / `spawn`). No detached tasks, no colored functions ([KDR-0002](docs/kdr/0002-no-async-await.md)).
- Tooling: one binary (`keel build|run|test|fmt|lint|audit|gen|fix`), zero config,
  non-configurable formatter and linter ([KDR-0010](docs/kdr/0010-one-formatter.md), [KDR-0018](docs/kdr/0018-waivers.md)).
- Deployment: static binaries for `FROM scratch`, cgroup-aware runtime,
  built-in `/healthz`, `/readyz`, SIGTERM drain, OpenTelemetry.
- Supply chain: package **capabilities** (`net`, `fs`, `exec`, `ffi`) enforced by the compiler ([KDR-0011](docs/kdr/0011-package-capabilities.md)).
- Evolution: Rust-style editions, hardened — old idioms become compile errors in
  new editions, and `keel fix` must migrate the public corpus automatically ([KDR-0001](docs/kdr/0001-editions.md)).

## What Keel is not for

Game engines, kernels, embedded, GUIs, sub-100µs deterministic latency.
Use Rust, C, or Zig there — Keel's FFI will call the result. See `docs/vision.md` §10.

## Repository map

| Path | Purpose |
|---|---|
| `docs/vision.md` | The design document (v0.2). Read this first. |
| `docs/spec/` | The normative language specification (in progress). |
| `docs/kdr/` | Keel Decision Records — every adopted/rejected decision, with reopening clauses. |
| `tests/conformance/` | Executable ground truth. The spec, as tests. **The most important directory for implementers.** |
| `compiler/` | The compiler (`keelc`). See `compiler/ARCHITECTURE.md` before writing code. |
| `examples/` | Idiomatic Keel programs the compiler must eventually accept. |
| `docs/milestone-status.md` | Per-milestone implementation status. |
| `ROADMAP.md` | Milestones, in dependency order. |
| `AGENTS.md` | Mandatory rules for LLM/agent contributors. |
| `CONTRIBUTING.md` | Rules for human contributors. |

## How implementation starts

Milestone 0 is not code. It is freezing **Keel Core** (`docs/spec/keel-core.md`) — the
minimal subset — and writing conformance tests for it. Every subsequent PR makes one
more conformance test pass. See `ROADMAP.md`.

## Current CLI (M4 snapshot)

The compiler builds two binaries from `compiler/keelc-driver`:

- `keel` — user-facing toolchain.
- `keelc` — conformance-runner entry point (also supports `check`/`run`).

Available commands:

```sh
cargo build --release -p keelc-driver

./target/release/keel run examples/hello.keel
./target/release/keel test tests/conformance/702-keel-test-runs-blocks/main.keel
./target/release/keel fmt tests/conformance/001-hello-world/main.keel
./target/release/keel build tests/conformance/001-hello-world/main.keel
```

`keel fmt` is the AST pretty-printer and is idempotent on the Keel Core
conformance corpus. `keel test` discovers `test "name" { assert expr }` blocks
and runs them. `keel build` compiles a Keel source file to a native binary
(placed next to the source file) via the Go toolchain.

## License

Apache-2.0. See [`LICENSE`](LICENSE) for details.
