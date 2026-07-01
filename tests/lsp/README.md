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

Definition, hover, completion, and document-symbol transcripts must land before
`keel lsp` advertises those capabilities in an implementation PR.
