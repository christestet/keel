# 16 — LSP Server Protocol

This chapter is **normative** for the `keelc-lsp` crate. It specifies which LSP
capabilities the server advertises, how compiler diagnostics map to protocol
messages, and the server's lifecycle contract. It does not restate the protocol
specification ([LSP 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/));
readers should be familiar with LSP.

Implementation governance: [`KDR-0103`](../kdr/0103-lsp-server.md). The server
is deferred to M7+; references to the salsa query core (target architecture)
are forward-looking and not yet implemented — see
[`compiler/ARCHITECTURE.md`](../../compiler/ARCHITECTURE.md) §Query-based core.

## 16.1 Capability advertisement

The server advertises these capabilities on `initialize`. Unlisted capabilities
are not supported and must not be requested by the client.

| LSP capability | Support | Notes |
|---|---|---|
| `TextDocumentSyncKind::Incremental` | M7+ | Full sync fallback; incremental after salsa core lands |
| `textDocument/publishDiagnostics` | M7+ | Maps `keelc` diagnostics to `Diagnostic` with `code` = `K####` |
| `textDocument/definition` | M7+ | Go-to-definition for identifiers |
| `textDocument/completion` | M7+ | Keyword and identifier completion |
| `textDocument/hover` | M7+ | Type display and doc comments |
| `textDocument/documentSymbol` | M7+ | Module-level symbol outline |
| `textDocument/references` | M8+ | Find all references |
| `textDocument/formatting` | M8+ | Invokes `keel fmt` on the document |
| `textDocument/codeAction` | M8+ | Quick fixes for known `K####` diagnostics |
| `workspace/symbol` | M8+ | Workspace-wide symbol search |
| `textDocument/rename` | M9+ | Identifier rename with semantics |
| `textDocument/inlayHint` | M9+ | Type hints on `let` bindings |

## 16.2 Diagnostics mapping

Every compiler diagnostic maps to an LSP `Diagnostic` as follows:

- **`range`**: computed from the diagnostic's `Span` (byte offset → line/column
  via the source map in `keelc-span`). The span is the primary error site, not
  the entire node.
- **`severity`**: `1` (Error) for errors, `2` (Warning) for warnings (codes
  `K04xx` wildcard-arm lint, etc.).
- **`code`**: the stable `K####` string, e.g. `"K0301"`.
- **`message`**: the diagnostic's rendered message. If the diagnostic carries
  a secondary span or a "help" note, it is appended after a newline, not as a
  `DiagnosticRelatedInformation` (to avoid client support variance — revisit
  when `relatedInformation` support is universal).
- **`source`**: `"keelc"`.

## 16.3 Server lifecycle

1. **`initialize`**: server receives workspace root, advertises capabilities
   per §16.1, and seeds the workspace state (optional, non-blocking).
2. **`initialized`**: server begins file-watching. Each open file is parsed and
   checked; diagnostics are pushed immediately.
3. **`textDocument/didChange`**: on each change, server re-checks the document
   and publishes fresh diagnostics. With the salsa core (target architecture),
   this is incremental; without it, the server re-checks the single file only
   (no cross-file dependency tracking).
4. **`textDocument/didClose`**: diagnostics are cleared for the closed document.
5. **`shutdown`**: server persists no state; clean exit.

## 16.4 Error handling

- Malformed LSP requests produce JSON-RPC error responses with code `-32600`
  (Invalid Request).
- Compiler panics on user input are a compiler bug — the server must not crash.
  The server wraps each compiler call in `catch_unwind` and returns a
  diagnostic-in-progress error to the client.
- If the compiler binary (`keelc`) is missing, the server logs a warning and
  serves only syntax-aware capabilities (brace matching, keyword completion
  from a built-in keyword list).

## 16.5 Implementation constraints

- No file system writes outside the workspace root.
- No background compilation threads that exceed one logical CPU.
- The server binary is `keel lsp` — a subcommand of the `keel` CLI, not a
  standalone binary.
- Dependencies: `tower-lsp`, `tokio`, `serde_json` — the only async/runtime
  dependencies in the compiler, justified by KDR-0103.
