# KDR-0040: `json.write` returns `String`

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** stdlib

## Decision

`json.write<T>(value: T) -> String`. It no longer returns
`Result<String, json.Error>`. Because `T` is constrained at compile time to a
JSON-representable type (`K1503`, §15.8), encoding is total: every representable
value has a canonical JSON text. A non-finite `Float` — the only previously
fallible case — serialises as the JSON literal `null`, matching the de-facto
`JSON.stringify` behaviour, rather than producing a runtime error.

## Context

The M6 exit program writes `http.created(json.write(user))` — the result feeds
directly into a response constructor that takes a `String`. With a `Result`
return, every write site needs a `catch` or `?` for an error that cannot occur
for a statically-checked representable type, which the §15.8 guard already
rules out. The `Result` shape existed only to model the non-finite `Float`
edge, and JSON has no number token for `NaN`/`±Infinity` regardless, so an
encoder must pick a total mapping; `null` is the established one.

## Alternatives considered

- **Keep `Result<String, json.Error>`.** Rejected: forces dead error handling
  at every call for a case `K1503` already excludes, and breaks the example's
  bare-call ergonomics — the whole reason the step exists.
- **`String` return, but `panic` on non-finite `Float`.** Rejected: turns a
  representable input into a crash; encoding a value the type system accepts
  must not abort.
- **Make the response constructors accept `Result<String, json.Error>`.**
  Rejected: pushes JSON error semantics into `std.http` and conflates two
  modules; the body of a response is a `String`.

## Consequences

`json.write` is a pure `String`-valued function; call sites drop their `catch`.
`json.Error` remains the error type of `json.parse`, which is genuinely
fallible. Non-finite floats round-trip as `null` (lossy, like every JSON
encoder); a future strict mode could reintroduce a checked writer without
changing this default. Spec: §15.7, §15.11, §15.12, §15.34.4. Conformance: the
existing `json.write` accept-cases drop their write-`catch`; behaviour and
output are unchanged for finite values.

## Reopening clause  *(required)*

Reopen if real Keel programs need to distinguish "serialised to `null` because
non-finite" from "serialised to `null` because `None`" at an API boundary
(measured, not hypothetical), which would justify a separate checked writer.
Other languages' choice of error-vs-`null` is not evidence.
