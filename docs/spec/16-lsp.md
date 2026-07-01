# 16 — LSP Server Protocol

This chapter is **normative** for the `keelc-lsp` crate. It specifies which LSP
capabilities the server advertises, how compiler diagnostics map to protocol
messages, and the server's lifecycle contract. It does not restate the protocol
specification ([LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/));
readers should be familiar with LSP.

Implementation governance: [`KDR-0103`](../kdr/0103-lsp-server.md). The server
is part of M8 and is backed by the Salsa query core accepted in
[`KDR-0106`](../kdr/0106-query-engine.md) — see
[`compiler/ARCHITECTURE.md`](../../compiler/ARCHITECTURE.md) §Query-based core.

## 16.1 Capability advertisement

The server advertises these capabilities on `initialize`. Unlisted capabilities
are not supported and must not be requested by the client.

| LSP capability | Support | Notes |
|---|---|---|
| `TextDocumentSyncKind::Incremental` | M8 base | Incremental document sync; no whole-workspace watcher requirement |
| `textDocument/publishDiagnostics` | M8 base | Maps `keelc` diagnostics to `Diagnostic` with `code` = `K####` |
| `textDocument/definition` | M8 base | Go-to-definition for identifiers |
| `textDocument/completion` | M8 base | Keyword and identifier completion |
| `textDocument/hover` | M8 base | Type display and doc comments |
| `textDocument/documentSymbol` | M8 base | Module-level symbol outline |
| `textDocument/references` | Deferred | Find all references |
| `textDocument/formatting` | Deferred | Invokes `keel fmt` on the document |
| `textDocument/codeAction` | Deferred | Quick fixes for known `K####` diagnostics |
| `workspace/symbol` | Deferred | Workspace-wide symbol search |
| `textDocument/rename` | Deferred | Identifier rename with semantics |
| `textDocument/inlayHint` | Deferred | Type hints on `let` bindings |

## 16.2 Diagnostics mapping

Every compiler diagnostic maps to an LSP `Diagnostic` as follows:

- **`range`**: computed from the diagnostic's `Span` as 0-based LSP positions
  with UTF-16 columns. Byte offsets are mapped through the source map in
  `keelc-span`; the span is the primary error site, not the entire node.
- **`severity`**: `1` (Error) for errors, `2` (Warning) for warnings (codes
  `K04xx` wildcard-arm lint, etc.).
- **`code`**: the stable `K####` string, e.g. `"K0301"`.
- **`message`**: the diagnostic's rendered message. If the diagnostic carries
  a secondary span or a "help" note, it is appended after a newline, not as a
  `DiagnosticRelatedInformation` (to avoid client support variance — revisit
  when `relatedInformation` support is universal).
- **`source`**: `"keelc"`.

## 16.3 Server lifecycle

1. **`initialize`**: server receives client capabilities and optional workspace
   roots, then advertises only the capabilities in §16.1.
2. **`initialized`**: server is ready to accept document notifications.
   Diagnostics are produced for opened documents.
3. **`textDocument/didChange`**: on each change, server re-checks the document
   through the Salsa query database and publishes fresh diagnostics.
4. **`textDocument/didClose`**: diagnostics are cleared for the closed document.
5. **`shutdown`**: server persists no state; clean exit.

## 16.4 Error handling

- Malformed LSP requests produce JSON-RPC error responses with code `-32600`
  (Invalid Request).
- Compiler panics on user input are a compiler bug — the server must not crash.
  The server wraps each compiler call in `catch_unwind` and returns a
  diagnostic-in-progress error to the client.
- Unsupported methods outside §16.1 produce JSON-RPC `-32601` (Method not
  found) responses.

## 16.5 Implementation constraints

- No file system writes outside the workspace root.
- No background compilation threads that exceed one logical CPU.
- The server binary is `keel lsp` — a subcommand of the `keel` CLI, not a
  standalone binary.
- Dependencies: `lsp-server`, `lsp-types`, `serde`, and `serde_json`, justified
  by KDR-0103. The M8 base server does not use `tower-lsp`, `tokio`, or another
  async runtime.
