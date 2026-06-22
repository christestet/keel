# ![Keel](keel-brand-kit/keel-logo-light-bg-256.png)

> A typed, compiled, garbage-collected language for backend services that should
> still be readable, reviewable, and deployable after five years of team churn.

**Status: M7 complete — 221 conformance cases pass and 4 earlier-milestone
rejection cases are intentionally skipped. M8 (incremental compiler core + LSP)
is scoped but not started.** See [`ROADMAP.md`](ROADMAP.md) and
[`docs/milestone-status.md`](docs/milestone-status.md).

## What Keel is

- One paradigm: procedural code over plain data, interfaces for polymorphism.
  No inheritance ([KDR-0003](docs/kdr/0003-no-inheritance.md)), no macros/reflection ([KDR-0004](docs/kdr/0004-no-macros.md)),
  no exceptions ([KDR-0005](docs/kdr/0005-no-exceptions.md)), no async/await ([KDR-0002](docs/kdr/0002-no-async-await.md)).
- Memory: concurrent low-latency GC + lexically scoped `arena` blocks ([KDR-0012](docs/kdr/0012-gc-plus-scoped-arenas.md)). No borrow checker.
- Safety: no null (`Option<T>`), no implicit zero values, exhaustive `match`,
  `Result` + `?` + `catch` for errors, union error types ([KDR-0005](docs/kdr/0005-no-exceptions.md)).
- Concurrency: structured only (`scope` / `spawn`). No detached tasks, no colored functions ([KDR-0002](docs/kdr/0002-no-async-await.md)).
- Tooling implemented today: `keel build|run|test|fmt|check|audit|gen`.
  `keel lsp` is planned for M8; `lint` and `fix` are not implemented.
- Deployment today: reproducible binaries emitted through the Go backend.
  Reproducible OCI images are planned for M9.
- Supply chain: package **capabilities** (`net`, `fs`, `exec`, `env`, `ffi`, `unsafe-memory`) enforced by the compiler ([KDR-0011](docs/kdr/0011-package-capabilities.md)).
- Evolution: Rust-style editions, hardened — old idioms become compile errors in
  new editions, and `keel fix` must migrate the public corpus automatically ([KDR-0001](docs/kdr/0001-editions.md)).

## What Keel is not for

Game engines, kernels, embedded, GUIs, sub-100µs deterministic latency.
Use Rust, C, or Zig there. Keel's C FFI is designed but not implemented; see
[`docs/vision.md`](docs/vision.md) §10 and
[`ROADMAP.md`](ROADMAP.md#m10--ecosystem-bridges).

## Repository map

| Path | Purpose |
|---|---|
| `docs/README.md` | Documentation map and reading paths. |
| `docs/vision.md` | The design document (v0.2): why Keel exists. |
| `docs/spec/` | The normative language specification (in progress). |
| `docs/kdr/` | Keel Decision Records — every adopted/rejected decision, with reopening clauses. |
| `tests/conformance/` | Executable ground truth. The spec, as tests. **The most important directory for implementers.** |
| `compiler/` | The compiler (`keelc`). See `compiler/ARCHITECTURE.md` before writing code. |
| `examples/` | Idiomatic Keel programs the compiler must eventually accept. |
| `docs/milestone-status.md` | Per-milestone implementation status. |
| `ROADMAP.md` | Milestones, in dependency order. |
| `AGENTS.md` | Mandatory rules for LLM/agent contributors. |
| `CONTRIBUTING.md` | Rules for human contributors. |

## Build the current toolchain

The compiler builds two binaries from `compiler/keelc-driver`:

- `keel` — user-facing toolchain.
- `keelc` — conformance-runner entry point (also supports `check`/`run`).

Available commands:

```sh
cargo build --release -p keelc-driver

./target/release/keel run examples/hello.keel --milestone M7
./target/release/keel check examples/hello.keel --milestone M7
./target/release/keel fmt examples/hello.keel --milestone M7
./target/release/keel build examples/hello.keel --milestone M7
```

The current development CLI defaults to the M1 parser gate, so use
`--milestone M7` for the complete implemented language. `keel fmt` writes
canonical source to stdout; it does not edit the input file. `keel build` writes
the executable beside the source file. Rust and Go are currently required to
build and run the compiler.

## License

Apache-2.0. See [`LICENSE`](LICENSE) for details.
