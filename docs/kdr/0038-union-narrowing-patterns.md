# KDR-0038: Union-narrowing patterns

- **Status:** proposed
- **Date:** 2026-06-21
- **Scope:** language
- **Amends:** [0005](0005-no-exceptions.md)

## Decision

Three pattern forms let a `match`/`catch` arm narrow a value whose static type
is a union (`A | B`) or a nested variant:

1. **Nested variant destructuring.** A pattern argument may itself be a pattern:
   `Err(NotFound(id))`, `Ok(Some(x))`. Each level matches a tag and binds the
   payloads of the next.
2. **Typed binding `x: T`.** Binds `x` and matches only when the value's runtime
   type is `T` — a union member. `Err(err: sql.Error)` selects the `sql.Error`
   arm of a `UserError | sql.Error`. When `T` is enumerable (an enum, a JSON/SQL
   classification set), the match tests tag membership; when `T` is opaque the
   binding matches the remainder (so it must come last, like a catch-all).
3. **Unit pattern `()`.** Matches the unit payload and binds nothing:
   `Ok(()) => ...` for `Result<Unit, E>`.

Exhaustiveness is unchanged: it is checked at the outer constructor level
(`Ok`/`Err`/`Some`/`None`, or the enum's variants), not driven into a union
payload. Arms are tried top to bottom; a typed binding for an opaque member is
the union's catch-all.

## Context

The M6 exit program returns `Result<User, UserError | sql.Error>` and, in its
HTTP handlers, must map each error class to a response: a domain `EmailTaken`
becomes a 409, a missing user a 404, and any database error a 500. That requires
destructuring the domain variants (`Err(EmailTaken(e))`), naming the opaque
member (`Err(err: sql.Error)`), and matching the unit success of `delete`
(`Ok(())`). Core already had `Ok`/`Err` single-level patterns; these three forms
are the minimal extension that lets one `match` cover a union without exposing
the opaque member's internals.

## Alternatives considered

- **Flat patterns only; branch with `if` on a classifier.** Rejected:
  abandons the exhaustive `match` that is Core's error-handling spine and
  scatters one decision across nested conditionals.
- **Drive exhaustiveness into the union payload.** Rejected for now: it would
  force every union `match` to enumerate members of an opaque type like
  `sql.Error`, which has no closed variant set (KDR-0037). Outer-level
  exhaustiveness plus ordered arms is sufficient and sound for the example.
- **A dedicated `is`/type-test expression instead of a pattern.** Rejected: a
  pattern composes with binding and nesting; a separate operator does not.

## Consequences

- `ast::Pattern::Name` gains an optional module `qualifier` and an optional type
  annotation `ty`; a `Pattern::Unit` variant is added. The KIR pattern
  distinguishes a binding (optionally with a tag-membership test) from a variant
  tag check, and the backend lowers a pattern to inline tag checks and
  `name := accessor` bindings recursively.
- Pattern bindings now carry their payload type into the arm scope, so a bound
  value is usable at its real type (`Ok(user)` makes `user` a `User`).
- No new diagnostic. Conformance: `795-union-narrowing-patterns` (run) covers
  nested destructuring, typed narrowing, and the unit pattern.

## Reopening clause  *(required)*

Reopen if outer-level exhaustiveness plus ordered arms is shown to admit a class
of silent mis-dispatch bugs (an arm unreachable or a member unhandled without a
diagnostic) that payload-level exhaustiveness would have caught, with a concrete
failing program. "Other languages narrow unions differently" is not evidence.
