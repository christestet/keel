# ![Keel](keel-brand-kit/keel-logo-light-bg-256.png)

> A typed, compiled, garbage-collected language for backend services that should
> still be readable, reviewable, and deployable after five years of team churn.

**Status: compiler under active development.** Start with
[`docs/vision.md`](docs/vision.md) and [`ROADMAP.md`](ROADMAP.md).

## What Keel is

- One paradigm: procedural code over plain data, interfaces for polymorphism.
  No inheritance, no macros, no reflection, no exceptions, no async/await.
- Memory: concurrent low-latency GC + lexically scoped `arena` blocks. No borrow checker.
- Safety: no null (`Option<T>`), no implicit zero values, exhaustive `match`,
  `Result` + `?` + `catch` for errors, union error types.
- Concurrency: structured only (`scope` / `spawn`). No detached tasks, no colored functions.
- Tooling: one binary (`keel build|run|test|fmt|lint|audit|gen|fix`), zero config,
  non-configurable formatter and linter (waivers are public and expire).
- Deployment: static binaries for `FROM scratch`, cgroup-aware runtime,
  built-in `/healthz`, `/readyz`, SIGTERM drain, OpenTelemetry.
- Supply chain: package **capabilities** (`net`, `fs`, `exec`, `ffi`) enforced by the compiler.
- Evolution: Rust-style editions, hardened — old idioms become compile errors in
  new editions, and `keel fix` must migrate the public corpus automatically.

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

## License

Apache-2.0. See [`LICENSE`](LICENSE) for details.
