# KDR-0036: Default function parameters

- **Status:** proposed
- **Date:** 2026-06-20
- **Scope:** language

## Decision

A function parameter may declare a default value: `limit: Int = 50`. The default
is an expression of the parameter's declared type. A call site that omits a
trailing argument receives the declared default in its place; arguments fill
parameters left to right, so an omitted argument is always a trailing one. A
parameter without a default still requires an argument.

Defaults change neither the signature's explicitness nor its types: parameter
and return types remain mandatory (`K0302`). A default does not introduce
inference — its type is the parameter's declared type, exactly as a struct field
default (`port: Int = 8080`) already works in Core.

This is the function-parameter analogue of the struct-field defaults the spec
already permits. The same rule applies: a defaulted parameter counts as provided
when its argument is omitted.

## Context

The M6 exit program declares `fn list_users(db: sql.Pool, limit: Int = 50)`. The
default documents the page size at the one place it is defined and lets callers
that do not care about pagination omit it. Core already supports defaults on
struct fields; not supporting them on parameters would be an arbitrary asymmetry
that forces either an overload (Core has none) or a sentinel value.

## Alternatives considered

- **No parameter defaults; require every argument.** Rejected: it contradicts
  the M6 example and the existing struct-field-default precedent for no reason.
- **Allow defaults in any position with named-argument skipping.** Rejected:
  Core's call arguments are positional (named arguments are labels in
  declaration order, not reordering), so only trailing omission is sound. A
  general skip mechanism is a larger feature the example does not need.
- **Infer the default's type.** Rejected: signatures are fully explicit; the
  default is checked against the declared type, never the source of it.

## Consequences

- The parser accepts `= expr` after a parameter's type; the formatter renders it
  and round-trips. The default expression is part of the parameter, not a
  statement, so it does not affect statement termination.
- Call lowering fills omitted trailing arguments with the callee's declared
  defaults before code generation, so the backend emits a complete argument
  list. No new runtime concept is introduced.
- Omitting an argument for a parameter that has no default remains an error in
  exactly the way a wrong-arity call already is; this KDR adds no new diagnostic.

## Reopening clause

Reopen only if corpus evidence shows positional-only defaults force a recurring,
awkward parameter ordering across at least three independent services that
named-argument reordering would demonstrably fix, and that reordering can be
specified without making call sites ambiguous. Preference for another language's
keyword-argument model is not evidence.
