# LSP protocol fixtures

This directory holds deterministic JSON-RPC transcript fixtures for the M8
`keel lsp` surface specified in [`docs/spec/16-lsp.md`](../../docs/spec/16-lsp.md)
and accepted by [`KDR-0103`](../../docs/kdr/0103-lsp-server.md).

These fixtures are protocol fixtures, not Keel language conformance cases. They
lock editor-visible LSP behavior: initialization capabilities, document
diagnostics, UTF-16 position mapping, shutdown, and JSON-RPC error handling.

## Format

Each fixture is a JSON object:

- `schema`: currently `keel-lsp-transcript/v1`.
- `case`: stable kebab-case fixture name.
- `description`: short human-readable purpose.
- `messages`: ordered client/server JSON-RPC messages.

Each message entry has:

- `direction`: `client` or `server`.
- `kind`: `request`, `response`, `notification`, or `raw-error`.
- `message`: a JSON-RPC object when the frame is valid JSON.
- `raw`: raw protocol input when the fixture intentionally uses malformed JSON.
- `expect`: the expected JSON-RPC response or notification.

Future runners must compare JSON objects byte-deterministically after canonical
serialization: sorted object keys, no insignificant whitespace, and exact string
contents. Position fixtures use escaped CRLF and escaped non-BMP Unicode so the
files remain ASCII while still locking LSP UTF-16 behavior.

## Current Coverage

- `001-initialize-shutdown.json`: initialization capability advertisement and
  clean shutdown.
- `002-publish-diagnostics-ascii.json`: diagnostic publication for an ASCII
  source span.
- `003-publish-diagnostics-utf16-crlf.json`: diagnostic publication for a CRLF
  document containing a non-BMP character before the error span.
- `004-error-responses.json`: malformed JSON and unsupported-method errors.
- `005-go-to-definition.json`: a call-site identifier resolves to its function
  declaration span.
- `006-hover-type-signature.json`: hover on a call-site identifier renders the
  resolved function signature.
- `007-completion-identifier.json`: completion on a partial identifier prefix
  offers a matching built-in function.
- `008-document-symbols-outline.json`: module-level struct fields and
  functions are enumerated with name selection ranges.
- `009-incremental-change-diagnostics.json`: an incremental
  `textDocument/didChange` edit re-checks the document and republishes
  diagnostics; reverting the edit clears them.
- `010-multiline-definition-position.json`: a call site many lines below its
  declaration resolves to the correct line, exercising multi-line position
  tracking beyond adjacent-line cases.

Every M8 base capability from spec chapter 16 now has at least one golden
transcript. An implementation PR must match these fixtures exactly before
`keel lsp` advertises the corresponding capability.
