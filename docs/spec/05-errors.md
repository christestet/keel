# 05 — Errors

This chapter is **normative**. It consolidates Keel's error model as it exists
after M7: the frozen Core rules of [`keel-core.md` §5](keel-core.md) plus the
decisions that completed the model. It does **not** restate the frozen rules
(`Result`/`?`, `catch` grammar, union signatures, `panic`); on any conflict
with `keel-core.md`, file an issue rather than reconciling silently (root
[`AGENTS.md`](../../AGENTS.md)). This chapter introduces **no new behavior and
no new diagnostic codes**.

The model in one paragraph
([KDR-0005](../kdr/0005-no-exceptions.md): no exceptions): errors are values in
`Result<T, E>`; `?` propagates them; `catch` handles them inline; unions type
"one of these failures"; opaque errors are classified, not destructured; the
universal `Error` absorbs everything at boundaries that only propagate; and
`panic` is reserved for bugs, with no catch mechanism.

## 5.1 Propagation: `?`

`?` unwraps `Ok`/`Some` or returns the error/`None` from the enclosing
function (cases `501`, `502`). Using `?` where the enclosing return type cannot
absorb the error is `K0501`, and the message names both types (case `503`).
`?` widens: an error may propagate into an enclosing union that contains it
(case `509`) or into `Error` (§5.4, case `511`). A `?` expression may appear
nested in a larger expression (case `799`) and as a discarded statement (case
`805`).

## 5.2 Inline handling: `catch`

`catch` over an **enumerable** error type must be exhaustive over its variants
or end in a propagating `other` arm (`K0502`, cases `504`, `505`, `508`).
Matching on a union must cover every member type (`K0503`, cases `506`, `510`);
union members narrow by typed binding patterns
([KDR-0038](../kdr/0038-union-narrowing-patterns.md), case `795`).

## 5.3 Opaque errors: classified, not destructured

An opaque error type (`sql.Error`,
[KDR-0037](../kdr/0037-sql-error-classification-patterns.md)) has no enumerable
variants. A `catch` over it uses qualified classification patterns
(`sql.NoRows`, `sql.UniqueViolation`), needs no exhaustive cover and no
`other` arm; an error matching no arm **propagates** through the enclosing
function, whose return type must absorb it (cases `796`, `806`). This is
`catch`'s analogue of `?`.

## 5.4 The universal `Error`

`Error` ([KDR-0033](../kdr/0033-universal-error-type.md)) is the opaque
boundary sink: any error type coerces into it at `?` and at tail/`return`
position of a function declaring `-> Result<T, Error>` (case `511`); it is the
only type that absorbs all others. It is renderable in interpolation but
cannot be destructured — matching on or `catch`-ing an `Error` value is
`K0504` (case `512`). Code that must branch on failure kind declares an
explicit union instead. Stdlib helpers accept `Error` where they only render
it ([KDR-0041](../kdr/0041-http-error-helpers-accept-error.md), case `798`).

## 5.5 Entry-point absorption

`fn main() -> Result<Unit, E>` exits nonzero and prints the error to stderr on
`Err` (Core §7); `E` is commonly `Error`, so any propagated error coerces in.

## 5.6 Error conditions

No codes are introduced here. The governing codes remain `K0501`–`K0504`
(Core §5), all compile-time diagnostics, never panics (root AGENTS.md hard
rule 6).

## 5.7 Conformance cases encoding this chapter

Already passing; this chapter adds no new cases: `501`–`512` (Core model),
`795`–`796` (union narrowing, opaque classification), `797`
(`Option.unwrap` as assertion, chapter 02 §2.2), `798`–`799`, `805`–`806`
(ergonomic positions, helper coercion).

## 5.8 Dependencies

- Frozen base: [`keel-core.md`](keel-core.md) §5, §7.
- Decisions: [KDR-0005](../kdr/0005-no-exceptions.md),
  [KDR-0033](../kdr/0033-universal-error-type.md),
  [KDR-0037](../kdr/0037-sql-error-classification-patterns.md),
  [KDR-0038](../kdr/0038-union-narrowing-patterns.md),
  [KDR-0039](../kdr/0039-option-unwrap.md),
  [KDR-0041](../kdr/0041-http-error-helpers-accept-error.md).
- Sibling chapters: [02](02-types.md) (union and `Error` forms),
  [04](04-expressions.md) (`match`), [15](15-stdlib-core.md) (`sql.Error`,
  HTTP helpers).
