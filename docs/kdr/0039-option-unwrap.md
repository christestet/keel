# KDR-0039: `Option<T>.unwrap()`

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** language

## Decision

`Option<T>` gains one method: `unwrap(self) -> T`. It returns the payload of a
`Some`; on `None` it aborts the program with a fixed runtime message. It takes
no arguments and is the only method on `Option`.

`unwrap` is a deliberate assertion, not error handling: the caller states the
value is present and accepts an abort if it is not. Recoverable absence uses
`match`, `?`, or `catch` as before; `unwrap` is for the case where `None` is a
bug, not a condition.

## Context

The M6 exit program (`examples/users-service/main.keel`) writes
`input.email.unwrap()` after it has already established the field is present.
Today only `Secret.unwrap()` exists (KDR-0030); `Option.unwrap` is `K0606`.
Every language with `Option`/`Maybe` provides this escape hatch, and Core needs
it so that "I know this is `Some`" does not force a `match` with an unreachable
arm.

## Alternatives considered

- **No `unwrap`; force `match`.** Rejected: produces an unreachable arm at every
  proven-present access and is the asymmetry KDR-0036 already rejected for
  defaults — Core should not punish the common assertion.
- **`unwrap_or(default)` only.** Rejected: does not express "this cannot be
  `None`"; a default silently masks the bug `unwrap` is meant to surface. Can be
  added later without conflict.
- **Return a sentinel / no abort on `None`.** Rejected: there is no sentinel for
  an arbitrary `T`, and silent fallthrough is the data-corruption mode the
  language exists to prevent.

## Consequences

`unwrap` on `None` is a runtime abort, parallel to division-by-zero (`K0204`)
and the existing `Secret.unwrap` shape — a program-level panic, not a compiler
panic, so the no-panic-on-user-*input* rule is unaffected. The compiler cannot
prove the call safe; that is the point. Spec: `keel-core.md` §2. Conformance:
`797-option-unwrap`.

## Reopening clause  *(required)*

Reopen if a corpus of real Keel programs shows `unwrap` is the dominant cause of
production aborts (i.e. it is being used for control flow rather than asserting
proven invariants), which would argue for a checked alternative
(`unwrap_or`/`expect`) as the default. Popularity of any other language's
`unwrap` policy is not evidence.
