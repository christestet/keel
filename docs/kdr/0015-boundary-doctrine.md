# KDR-0015: Boundary doctrine — parse, don't validate, strict default

- **Status:** accepted
- **Scope:** language

## Decision

External data enters only through explicit parse points (`json.parse<T>`,
`proto.decode<T>`, `sql` row mapping). Parsing is strict by default (unknown
fields are errors) with an explicit relaxation: `json.parse<T>(body, mode:
.tolerant)`. Tolerance is a greppable, alertable choice.

## Context

Derived from [`docs/vision.md`](../vision.md) §6. The "parse, don't validate"
principle makes illegal states unrepresentable inside a Keel program while
acknowledging that the network does not care about your types.

## Reopening clause

Corpus evidence that strict-by-default causes measurable operational pain that
.tolerant cannot address, or bug data showing .tolerant being used as a silent
default.
