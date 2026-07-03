# KDR-0037: sql.Error classification patterns and catch propagation

- **Status:** accepted
- **Date:** 2026-06-21
- **Scope:** language
- **Amends:** [0029](0029-sql-database-access.md), [0033](0033-universal-error-type.md)

## Decision

`sql.Error` stays **opaque** — it has no public, enumerable variant set and
cannot be destructured. It is *classified*, not matched: a `catch`/`match` arm
may name a qualified classification pattern `sql.NoRows`, `sql.UniqueViolation`
(and the other documented classifications) to branch on the kind of failure.

Because the classification set is not closed, a `catch` over `sql.Error` (or a
union containing it) is **not** required to be exhaustive and **needs no `other`
arm**. An error that matches no arm **propagates**: it is re-wrapped and
returned through the enclosing function, whose return type must absorb it (an
`... | sql.Error` union, or `Error`). This is the catch analogue of `?`.

```keel
fn get_user(db: sql.Pool, id: Uuid) -> Result<User, UserError | sql.Error> {
    let row = db.query_one("select ... where id = $1", id)
        catch sql.NoRows => return Err(NotFound(id))   // any other sql error propagates
    Ok(User.from_row(row)?)
}
```

The arrow form `catch <pattern> => expr` accepts a classification pattern as its
head and binds nothing. The brace form may mix classification patterns with a
final `other`/`_` catch-all, in which case nothing propagates.

## Context

The M6 exit program classifies driver failures: a unique-constraint violation
becomes a domain `EmailTaken`, a missing row becomes `NotFound`, and every other
database error is surfaced unchanged. A closed `sql.Error` enum would force the
program to enumerate every driver/SQLSTATE condition — exactly the brittle,
leaky coupling [0029](0029-sql-database-access.md) keeps behind the opaque type.
Classification + propagation gives the two branches the program actually wants
without exposing the rest.

## Alternatives considered

- **Closed `sql.Error` enum.** Rejected: ties Core to a driver's error taxonomy
  and breaks every `match` when a classification is added — the coupling
  [0029](0029-sql-database-access.md) exists to prevent.
- **Require an `other`/`_` arm always.** Rejected: the common case (`catch
  sql.NoRows => ...`) wants the rest to propagate, exactly like `?`. Forcing a
  ceremonial `other => return Err(other)` adds noise without meaning.
- **Make classification a runtime method (`err.is_unique_violation()`).**
  Rejected: it abandons the pattern syntax the rest of Core uses for error
  branching and cannot bind in a `match` arm.

## Consequences

- The parser accepts qualified patterns (`sql.NoRows`) in catch/match arms and a
  classification pattern as a `catch ... =>` head (KDR-0038 covers the shared
  pattern-grammar extension).
- Lowering appends a propagating arm (`return Err(<error>)`) to a catch over an
  opaque error type when no catch-all is present; a closed-enum catch is
  unchanged and still exhaustiveness-checked (`K0502`).
- The backend matches a classification by its runtime tag; no new diagnostic.
- Validating a classification end-to-end needs a live database, so the
  conformance case (`796-sql-classification-catch`) is build-mode; the running
  proof is the M6 exit harness (Step 6).

## Reopening clause  *(required)*

Reopen if conformance or field use shows the propagation default hides errors a
program meant to handle — i.e. a measured class of bugs where an unmatched
classification silently propagated past code that should have branched on it —
or if a closed, driver-independent `sql.Error` taxonomy is demonstrated that
does not couple Core to a specific backend. Popularity of exhaustive-only error
handling is not evidence.
