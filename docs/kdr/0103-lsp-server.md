# KDR-0103: LSP Server — protocol-driven editor integration

- **Status:** proposed
- **Date:** 2026-06-14
- **Scope:** toolchain

## Decision

Build a Language Server Protocol (LSP) server as a new compiler crate
(`keelc-lsp`) that exposes `keel check` diagnostics, go-to-definition,
completions, hover type/doc info, and document symbols through the standardized
LSP interface. The server connects to the compiler's future salsa-style query
database for incremental responses. Deferred to **M7+** — no LSP work begins
until the toolchain skeleton (M4) and language completion wave 1 (M5) exit
criteria are met, and the salsa-based incremental core is operational.

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

The Rust LSP ecosystem is mature. The [`tower-lsp`](https://github.com/ebkalderon/tower-lsp)
crate provides an async LSP framework built on `tower` and `tokio`, used by
several production language servers. It handles protocol negotiation, JSON-RPC
transport, and capability advertisement — Keel's server implements only the
backend callbacks.

## Alternatives considered

- **Per-editor plugins (VS Code extension, Neovim plugin, etc.).** Rejected:
  fragments integration knowledge into N implementations. Each gains niche
  features the others lack; none is testable in isolation. LSP is the
  industry-agreed solution to this exact problem.
- **No editor integration beyond `keel check`.** Rejected: `keel check` with
  `:make` / `compiler` plugins is the minimum viable path but yields no
  go-to-definition, no completions, no hover docs — the features teams expect
  from a modern language.
- **Shelling out to `keelc check` on each LSP event.** Rejected for M7+
  production; acceptable as an M5/M6 scaffolding path. The final server must
  maintain an in-process compiler database for sub-ms queries.
- **Embedding a full `keelc` as a library.** Accepted — this is the plan.
  The driver library (`keelc-driver`) already separates CLI from logic; the
  LSP crate calls into the same pipeline.

## Consequences

- A new `keelc-lsp` crate is added to the workspace, depending on `tower-lsp`,
  `tokio`, `serde_json`, and the existing `keelc-driver` library.
- `tower-lsp` and `tokio` are the first async/runtime dependencies in the
  compiler — this KDR explicitly justifies them (per hard rule 5 in
  [`AGENTS.md`](../../AGENTS.md)).
- The LSP binary (`keel lsp`) runs as a long-lived daemon. It must handle
  workspace open/close, file change notifications, and cancellation.
- `keel check` output must be structured (diagnostic codes, spans, and messages)
  for reliable LSP mapping — this is already the design (see
  [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) §Diagnostics).
- The salsa query core (target architecture) becomes a hard dependency for
  LSP performance: every keystroke triggers a re-check of the affected file
  and its dependents. Without incrementality, the LSP server cannot meet the
  vision.md §7 budget.
- The crate layout grows by one entry:

  ```
  keelc-lsp/   LSP server — protocol handlers, workspace state, capability table
  ```

## Reopening clause

Evidence that the Rust LSP ecosystem (`tower-lsp`) imposes unacceptable
constraints (e.g., performance ceiling, maintenance burden, or licensing
conflict) sufficient to justify either a custom LSP implementation or a
non-LSP editor integration strategy. Also reopenable if the salsa query core
proves unable to meet the < 300 ms `keel check` budget with LSP overhead.
