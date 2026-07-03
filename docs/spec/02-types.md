# 02 — Types

This chapter is **normative**. It consolidates Keel's type system as it exists
after M7: the frozen Core rules of [`keel-core.md` §2](keel-core.md) plus the
type forms later chapters and KDRs added on top. It does **not** restate the
frozen rules (primitive set, no null, no implicit conversions, overflow
behavior); on any conflict with `keel-core.md`, file an issue rather than
reconciling silently (the prime directive in the root
[`AGENTS.md`](../../AGENTS.md)). This chapter introduces **no new behavior and
no new diagnostic codes** — every statement below is already encoded by the
cited conformance cases.

## 2.1 The type forms

A Keel type is exactly one of:

| Form | Examples | Introduced by |
|---|---|---|
| Primitive | `Int`, `Float`, `Bool`, `String`, `Char`, `Unit` | Core §2 |
| Built-in generic | `Option<T>`, `Result<T, E>`, `List<T>`, `Map<K, V>` | Core §2 |
| Named struct/enum | `User`, `Status` | Core §3, chapter [03](03-declarations.md) |
| Boundary scalar | `Uuid`, `Timestamp`, `Email` | [`15-stdlib-core.md`](15-stdlib-core.md), [KDR-0034](../kdr/0034-core-boundary-scalars.md) |
| Interface | `Printable` (as a parameter/return type) | chapter [07](07-interfaces.md) |
| Constrained type parameter | `T` with an interface bound | chapter [08](08-generics.md) |
| Union (error position) | `DbError \| ParseError` | Core §5, chapter [05](05-errors.md) |
| Universal error | `Error` | [KDR-0033](../kdr/0033-universal-error-type.md), chapter [05](05-errors.md) |

There are no other type forms: no pointers, no references, no nullable types,
no user-defined generic *types* beyond struct/enum declarations with bounded
parameters (chapter 08), no structural/anonymous types.

## 2.2 Absence is `Option<T>`

`Option<T>` (`Some(T)` / `None`) is the only representation of absence (case
`004`); any nullish construct is `K0201` (case `103`). `Option<T>.unwrap() -> T`
returns the `Some` payload and aborts on `None`
([KDR-0039](../kdr/0039-option-unwrap.md), case `797`) — it is an assertion,
not error handling; recoverable absence uses `match`, `?`, or `catch`.

## 2.3 No implicit conversions

There are no implicit conversions between any two types. Mixed-operand
arithmetic (`Int` + `Float`) is `K0202` (cases `101`, `120`); conversion is
explicit (`Float.from(i)`, cases `102`, `110`). Equality between `Int` and
`Float` values requires the same explicit conversion (case `109`). The full
operator/overflow semantics, including `K0203`/`K0204` and division behavior,
are chapter [04](04-expressions.md) and
[KDR-0013](../kdr/0013-core-operators-and-integer-division.md).

## 2.4 Type equivalence and coercion

Type equivalence is **nominal**: two types are the same only if they are the
same primitive/built-in instantiated with equivalent arguments, or the same
declaration ([KDR-0003](../kdr/0003-no-inheritance.md): there is no subtyping
hierarchy). Exactly two widenings exist, both in error position and both
defined by chapter [05](05-errors.md):

1. a type may widen into a union that contains it (case `509`);
2. any error type coerces into the universal `Error` at `?` and at
   tail/`return` position (case `511`).

A value of interface type accepts any type with a declared `impl` for that
interface (chapter [07](07-interfaces.md), case `215`) — explicit declaration,
not structural matching (case `225` rejects structural satisfaction without an
`impl`).

## 2.5 Inference boundary

Type inference is local only: `let x = expr` infers the binding's type from
`expr` (case `202`). Signatures never infer — parameter and return types are
required (`K0302`, case `201`); struct fields and enum payloads are always
explicitly typed. There is no cross-function, cross-file, or bidirectional
inference. Call-site type arguments, where needed, use `<T>`
([KDR-0032](../kdr/0032-call-site-type-args.md)).

## 2.6 Error conditions

No codes are introduced here. The governing codes remain: `K0201` (nullish
construct, Core §2), `K0202` (implicit conversion, Core §2 / chapter 04),
`K0203`/`K0204` (overflow family, chapter 04), `K0302` (missing signature
types, Core §3 / chapter 03).

## 2.7 Conformance cases encoding this chapter

Already passing; this chapter adds no new cases: `002`, `004`, `101`–`103`,
`109`–`111`, `120` (primitives, no-null, explicit conversion), `201`–`202`
(inference boundary), `212`–`234` (interface and type-parameter forms, chapter
07/08 tables), `501`–`512` (union and `Error` forms, chapter 05 table),
`779`–`793` (boundary scalars, chapter 15).

## 2.8 Dependencies

- Frozen base: [`keel-core.md`](keel-core.md) §2.
- Decisions: [KDR-0003](../kdr/0003-no-inheritance.md),
  [KDR-0009](../kdr/0009-no-operator-overloading.md),
  [KDR-0013](../kdr/0013-core-operators-and-integer-division.md),
  [KDR-0032](../kdr/0032-call-site-type-args.md),
  [KDR-0033](../kdr/0033-universal-error-type.md),
  [KDR-0034](../kdr/0034-core-boundary-scalars.md),
  [KDR-0039](../kdr/0039-option-unwrap.md).
- Sibling chapters: [03](03-declarations.md) (declared types),
  [04](04-expressions.md) (operators over these types),
  [05](05-errors.md) (unions, `Error`), [07](07-interfaces.md),
  [08](08-generics.md), [15](15-stdlib-core.md) (boundary scalars).
