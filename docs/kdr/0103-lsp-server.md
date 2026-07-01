# KDR-0103: LSP Server — protocol-driven editor integration

- **Status:** accepted
- **Date:** 2026-07-01
- **Scope:** toolchain

## Decision

Build a Language Server Protocol (LSP) server as a new compiler crate
(`keelc-lsp`) exposed only through the `keel lsp` subcommand. The M8 base
server advertises exactly these LSP 3.17 capabilities:

- incremental text-document synchronization;
- diagnostics with stable `K####` codes;
- go-to-definition;
- completion;
- hover;
- document symbols.

References, formatting, code actions, workspace symbols, rename, inlay hints,
and semantic tokens are deferred beyond M8 unless a later KDR and spec change
pull one of them forward.

The server must be backed by the same in-process Salsa query graph accepted in
[`KDR-0106`](0106-query-engine.md). It must not shell out to `keelc check` on
document changes, and it must not introduce a second parser, resolver,
typechecker, formatter, or diagnostic renderer. The query surface may be exposed
from `keelc-driver` or moved into a separately justified compiler crate, but
protocol handlers must consume the same pure stage outputs used by `keel check`.

Use the synchronous Rust LSP stack:

- `lsp-server` for JSON-RPC framing, stdio transport, initialization, and the
  explicit dispatch loop;
- `lsp-types` for protocol data structures;
- `serde` and `serde_json` for request/response encoding.

Do not add `tower-lsp`, `tokio`, or another async runtime for the M8 base
server. The M8 server has one client connection over stdio and a deliberately
small capability set; an explicit synchronous loop is easier to keep
deterministic, test with golden transcripts, and bound to one logical CPU.

The LSP server is explicitly **not** a language feature. It is a toolchain
server that speaks a wire protocol; it must not drive compiler design decisions
nor create new surface area in the language.

## Context

Derived from [`docs/vision.md`](../vision.md) §7 (Compile time as a contract):
`keel check` is the designated editor fast path (< 300 ms). An LSP server is the
standard way to surface that fast path in editors — it watches files, delivers
diagnostics on save, and answers queries without the user leaving their editor.

An LSP server is a prerequisite for any serious adoption of Keel in a
team environment. The protocol (LSP 3.17) is an industry standard supported by
VS Code, Neovim, Helix, Zed, JetBrains, and Emacs, making editor support a
one-time implementation cost rather than per-editor drift. Precedent: every
modern language that achieved editor integration without a dedicated team built
an LSP server first (Go's `gopls`, Rust's `rust-analyzer`, TypeScript's `tsserver`,
Zig's `zls`).

The Rust LSP ecosystem has two viable directions. [`tower-lsp`](https://docs.rs/tower-lsp/latest/tower_lsp/)
provides an async service abstraction with Tokio integration. [`lsp-server`](https://docs.rs/lsp-server/latest/lsp_server/)
provides a synchronous crossbeam-channel-based scaffold derived from the
rust-analyzer ecosystem: it handles protocol handshaking and message parsing
while the language server owns dispatch. Keel chooses `lsp-server` for M8
because the base server needs deterministic request handling and transcript
fixtures more than async service composition.

## Alternatives considered

- **Per-editor plugins (VS Code extension, Neovim plugin, etc.).** Rejected:
  fragments integration knowledge into N implementations. Each gains niche
  features the others lack; none is testable in isolation. LSP is the
  industry-agreed solution to this exact problem.
- **No editor integration beyond `keel check`.** Rejected: `keel check` with
  `:make` / `compiler` plugins is the minimum viable path but yields no
  go-to-definition, no completions, no hover docs — the features teams expect
  from a modern language.
- **Shelling out to `keelc check` on each LSP event.** Rejected for M8. It would
  create a second observable driver path, lose query reuse, and make the editor
  latency budget depend on process startup. The server must
  maintain an in-process compiler database for sub-ms queries.
- **Embedding compiler stages as a library.** Accepted. The LSP crate calls the
  same query-backed compiler pipeline as the CLI, with filesystem/process
  effects kept out of query functions.
- **`tower-lsp` plus `tokio`.** Rejected for M8. It is a capable stack, but it
  would make async/runtime dependencies part of the compiler before Keel has a
  server workload that needs them. Reopen if transcript testing, cancellation,
  or client compatibility shows the synchronous stack is the wrong boundary.
- **Custom JSON-RPC implementation.** Rejected: parsing/framing LSP messages is
  protocol plumbing, not a Keel differentiator.

## Consequences

- A future implementation PR may add a `keelc-lsp` crate to the workspace and
  may add direct dependencies on `lsp-server`, `lsp-types`, `serde`, and
  `serde_json`. Their transitive dependencies are justified by this KDR, but
  unrelated dependencies are not.
- The LSP binary (`keel lsp`) runs as a long-lived daemon. It must handle
  workspace open/close, file change notifications, request cancellation where
  the protocol requires it, and shutdown.
- `keel check` output must be structured (diagnostic codes, spans, and messages)
  for reliable LSP mapping — this is already the design (see
  [`compiler/ARCHITECTURE.md`](../../compiler/ARCHITECTURE.md) §Diagnostics).
- The Salsa query core is a hard dependency for LSP performance: every document
  change invalidates source inputs and reuses unaffected query results. Without
  incrementality, the LSP server cannot meet the vision.md §7 budget.
- Protocol behavior must be locked with deterministic JSON-RPC transcripts
  before large protocol handlers land. Transcript tests are the conformance
  equivalent for the LSP surface.
- The crate layout grows by one entry:

  ```
  keelc-lsp/   LSP server — protocol handlers, workspace state, capability table
  ```

## Reopening clause

Reopen this decision if one of the following is demonstrated:

- `lsp-server` prevents correct LSP 3.17 behavior, request cancellation,
  transcript testing, or common editor interoperability;
- the synchronous dispatch loop cannot meet the M8 latency budget even after
  query-level profiling identifies protocol handling, rather than compiler
  stage work, as the blocker;
- an async stack such as `tower-lsp` materially reduces implementation risk
  without violating determinism, one-CPU, and dependency-discipline constraints;
- the Salsa query core proves unable to meet the < 300 ms `keel check` budget
  with LSP overhead.
