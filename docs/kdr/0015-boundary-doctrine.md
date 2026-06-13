# KDR-0015: Boundary doctrine — parse, don't validate, strict default

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

External data enters only through explicit parse points (`json.parse<T>`,
`proto.decode<T>`, `sql` row mapping). Parsing is strict by default (unknown
fields are errors) with an explicit relaxation: `json.parse<T>(body, mode:
.tolerant)`. Tolerance is a greppable, alertable choice — `mode: .tolerant`
ignores unknown fields and logs a structured `schema_drift` event to OTel.
Tolerance is not the default.

The type `T` must honestly describe the wire reality: any field the schema does
not guarantee is `Option<T>`. The compiler rejects a required field for
optional wire data when a schema is available to check against.

## Context

Derived from [`docs/vision.md`](../vision.md) §6. The "parse, don't validate"
principle makes illegal states unrepresentable inside a Keel program while
acknowledging that the network does not care about your types.

The ergonomic key is that developers usually do not hand-write boundary types:
`keel gen` (KDR-0020) derives them from proto/OpenAPI sources of truth, so the
honest-`Option` rule costs nothing in typing. The "five-year-old API with
misspelled fields" case is handled by a generated type plus one `.tolerant` in
the parse call.

## Alternatives considered

- **Silent tolerance** (Go's `json.Unmarshal` default — unknown fields are
  silently ignored). Rejected: causes silent contract drift. A field rename in
  the API becomes a silently missing value, not an error. The `schema_drift`
  event in `.tolerant` mode provides observability without breaking production.

- **Strict-only, no tolerance** (default serde behaviour in Rust). Rejected:
  real-world operational pain. API contracts evolve, canaries run mixed
  versions, and a hard-reject on unknown fields prevents staged rollouts.
  Tolerance is a legitimately needed escape hatch, but it must be explicit.

- **Dynamic typing at the boundary** (Python, JavaScript, Ruby approach —
  parse to `dict`/`object` and access fields dynamically). Rejected: defeats
  Keel's core safety proposition. A boundary type `T` with `Option` fields is
  the minimal honest contract.

## Consequences

- Contract drift is caught in dev (strict mode errors) and observable in
  production (`.tolerant` with OTel `schema_drift` events).
- Unknown-field typos are compile-time errors in tests (strict mode) and
  observable warnings via `schema_drift` in production.
- Generated types from `keel gen` are automatically honest about optionality:
  if the proto field is `optional` or the OpenAPI property has no `required`,
  the generated Keel field is `Option<T>`.
- The boundary between "inside the program" and "outside the program" is always
  an explicit parse call, making audit of injection surfaces straightforward.

## Reopening clause

Corpus evidence that strict-by-default causes measurable operational pain that
`.tolerant` cannot address, or bug data showing `.tolerant` being used as a
silent default (indicating the default should be re-examined).
