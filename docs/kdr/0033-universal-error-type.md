# KDR-0033: Universal `Error` type

- **Status:** proposed
- **Date:** 2026-06-20
- **Scope:** language

## Decision

`Error` is a compiler-known, opaque universal error type. Any error type `E`
coerces into `Error` implicitly wherever an `Error` is expected — at `?`
propagation and at tail/`return` position in a function declaring
`-> Result<T, Error>`. `Error` carries a human-readable message (it is
renderable via string interpolation / `Display`) but is **opaque**: it has no
matchable variants and cannot be destructured, pattern-matched, or `catch`-ed
into cases. Code that must branch on which error occurred declares an explicit
union error type (`A | B`, KDR-0005); `Error` is exclusively a *boundary sink*
for code that only propagates. `Error` is the only error type that absorbs all
others; concrete and union error types retain their exhaustiveness obligations
unchanged.

## Context

The M6 exit program ([`examples/users-service/main.keel`](../../examples/users-service/main.keel))
declares `fn main() -> Result<Unit, Error>` and `?`-propagates three distinct
error types into it (`config.Error`, `sql.Error`, `http.Error`) without ever
matching them. This is the canonical top-level shape: a boundary that, on any
failure, terminates the process non-zero and prints a diagnostic. Enumerating a
`config.Error | sql.Error | http.Error` union at `main` would be pure
signature noise — `main` never branches on the difference.

KDR-0005 deliberately requires functions to *declare exactly which error types
they produce* so callers handle each or propagate explicitly, and requires
unions to be matched exhaustively (`K0503`). That guarantee is valuable
precisely because callers act on the distinction. At a pure boundary
(`main`, a top-level request handler that maps everything to `500`) there is no
distinction to act on, and the enumeration is friction without payoff. `Error`
fills exactly that gap — and only that gap.

The opacity rule is load-bearing: an `Error` you could destructure would
reintroduce the "I got some error, let me guess what it is" pattern KDR-0005
exists to prevent. By making `Error` write-only (coerce in, render out, never
match), the two mechanisms stay cleanly separated: **unions where you branch,
`Error` where you don't.**

Prior art: Rust's `Box<dyn std::error::Error>` / `anyhow::Error` are the same
"universal boundary error" idea, and they cost exactly what the opacity rule
forbids — `downcast` turns them back into an untyped guessing game. Go's bare
`error` interface is universal but *only* matchable by convention
(`errors.As`), which is the same hazard. Keel takes the ergonomic win
(universal coercion at boundaries) while refusing the hazard (no downcast, no
destructure).

## Alternatives considered

- **No universal type; require a union at `main`.** Rejected: forces
  `Result<Unit, config.Error | sql.Error | http.Error>` (and growth on every
  new stdlib error) at a site that never inspects the union. Maximises
  signature churn for zero reasoning benefit. The exhaustiveness guarantee it
  preserves is worthless where nobody branches.

- **Make `Error` destructurable (downcast / variant match).** Rejected: this is
  the `anyhow::downcast` / `errors.As` hazard. It silently reintroduces
  unchecked error guessing and defeats KDR-0005's reason for existing. If you
  need to branch, you already have the right tool — a union — and you must use
  it.

- **`Error` as a stdlib interface users implement** (Go's `error`). Rejected
  for M6: an interface invites bespoke error types and `errors.As`-style
  matching, the exact pattern above. A closed, compiler-known opaque type has
  no such surface. An interface-based extensibility story can be reopened later
  if the corpus demands user-defined boundary errors.

- **Implicit coercion everywhere, not just into `Error`.** Rejected: implicit
  widening among *concrete* error types would erode the "declare exactly what
  you produce" rule. Coercion is permitted into `Error` and nowhere else; the
  target type `Error` is the explicit, visible opt-in to opacity.

## Consequences

- `main` and other pure boundaries get an ergonomic, churn-free signature:
  `-> Result<Unit, Error>`. Adding a new propagated error type never changes
  the signature.
- The `?` operator and tail-return checking gain one absorption rule: every `E`
  is assignable to `Error`. This generalises the existing union-widening path
  rather than adding a parallel mechanism.
- You **cannot** recover differently based on an `Error`'s origin. The moment
  you need to, you must change the signature to a union and handle it
  exhaustively — the type system pushes you to the honest declaration. This is
  a deliberate inconvenience aimed at code that would otherwise guess.
- A new diagnostic is required for "attempted to `match`/`catch`/destructure a
  value of type `Error`" (next free `K####`, registered append-only in
  `keelc-diag`, encoded as a reject-case in its own PR).
- Runtime: `Error` reuses the existing `KeelEnum` error carrier; rendering an
  `Error` yields the underlying error's message. No new runtime representation.

## Reopening clause

Reopen if corpus evidence shows boundary code that genuinely must recover
differently based on an `Error`'s origin *and* cannot reasonably name the
union it came from — for example, a framework-level handler that receives
errors from open-world plugins whose concrete error types are not knowable at
the handler's definition site. A demonstrated, non-contrived case where opacity
forces strictly worse code than a union would (not mere preference for
`downcast`) reopens the opacity rule. Advocacy, `anyhow` familiarity, and
"Go/Rust allow downcast" are not sufficient.
