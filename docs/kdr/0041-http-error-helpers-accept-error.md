# KDR-0041: HTTP error response helpers accept `Error`

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** stdlib

## Decision

The two error-status response constructors take the universal `Error` type
instead of `String`:

```text
fn http.bad_request(err: Error) -> http.Response    — 400 Bad Request
fn http.internal_error(err: Error) -> http.Response — 500 Internal Server Error
```

`Error` (KDR-0033) absorbs every error type, so any `json.Error`, `sql.Error`,
`http.Error`, or user error flows in directly, as does a `String` (which
`Error` also absorbs). The body is the error's rendered text — the same
rendering string interpolation produces for an `Error`. The success/neutral
constructors (`ok`, `created`, `conflict`, `no_content`, `not_found`) keep their
existing `String`/no-argument signatures: their bodies are application payloads,
not errors.

## Context

In the M6 exit program a handler turns a failed operation straight into a
response: `json.parse<CreateUser>(req.body) catch err => http.bad_request(err)`
and `Err(err: sql.Error) => http.internal_error(err)`. With a `String`
parameter every such site must first stringify the error by hand, which is
boilerplate at exactly the boundary where errors are most common. `Error` is
already the language's "any error" sink and is renderable but not
destructurable, which is precisely what a 400/500 body needs.

## Alternatives considered

- **Keep `String`; stringify at each call.** Rejected: repeats
  `"{err}"`-style conversion at every error boundary for no gain.
- **Overload each helper for `String` and `Error`.** Rejected: Core has no
  overloading, and `Error` already absorbs `String`, so one signature covers
  both.
- **Add a dedicated `http.Error`-shaped body.** Rejected: invents a parallel
  error channel when `Error` already unifies all of them.

## Consequences

Error responses are one call with no manual conversion. How an `Error`
stringifies into a body is the existing `Error` rendering (debug-grade for now);
a future change to that rendering improves these bodies for free. `bad_request`
and `internal_error` can no longer reject a non-`String` argument at the type
level — but `Error` absorbing everything is the intended boundary behaviour.
Spec: §15.20. Conformance: `798-http-error-helper-accepts-error`.

## Reopening clause  *(required)*

Reopen if production use shows error responses need structured bodies (e.g. a
machine-readable error envelope) rather than rendered `Error` text, which would
justify a typed response body instead of a stringified `Error`. A preference
for one framework's error-body convention is not evidence.
