# 03 — Declarations

This chapter is **normative**. It consolidates Keel's declaration forms as they
exist after M7: the frozen Core rules of [`keel-core.md` §3](keel-core.md) plus
the forms later chapters and KDRs added. It does **not** restate the frozen
rules (struct/enum/fn grammar, no zero values, explicit signatures, `let`/`mut`);
on any conflict with `keel-core.md`, file an issue rather than reconciling
silently (root [`AGENTS.md`](../../AGENTS.md)). This chapter introduces **no
new behavior and no new diagnostic codes**.

## 3.1 The declaration forms

A module (one `.keel` file, [`06-modules-packages.md`](06-modules-packages.md))
declares, at the top level, any number of:

| Form | Governed by |
|---|---|
| `struct` | Core §3; field defaults below |
| `enum` (variants with optional named payloads) | Core §3 |
| `fn` | Core §3; default parameters below |
| `interface` (≤ 5 methods) | chapter [07](07-interfaces.md), [KDR-0003](../kdr/0003-no-inheritance.md) |
| `impl <Interface> for <Type>` | chapter [07](07-interfaces.md), [KDR-0023](../kdr/0023-impls-on-primitive-types.md) |
| `test "name" { }` | Core §7, chapter plan 13 |
| `use` / `module` header | chapter [06](06-modules-packages.md) |

There are no other top-level forms: no inheritance or subclassing
([KDR-0003](../kdr/0003-no-inheritance.md)), no macros
([KDR-0004](../kdr/0004-no-macros.md)), no attributes/annotations (`K0906`,
case `906`), no operator-overload declarations (`K0907`,
[KDR-0009](../kdr/0009-no-operator-overloading.md), case `907`), no global
variables, no conditional compilation
([KDR-0006](../kdr/0006-no-conditional-compilation.md)).

Naming is enforced, not conventional: type names are `UpperCamelCase`, value
and function names `snake_case` (`K0101`, cases `006`, `007`).

## 3.2 Struct fields and defaults

Construction requires every field (`K0301`, no zero values — cases `002`,
`210`, `301`). A field may declare a default of its declared type
(`port: Int = 8080`), which counts as provided (cases `206`, `211`). An
`Option` field is still explicit: absent means writing `None`, never omitting
the field (case `207`).

## 3.3 Function signatures and default parameters

Parameter and return types are required (`K0302`, case `201`); `-> Unit` may be
omitted (case `205`). A parameter may declare a default value of its declared
type (`limit: Int = 50`, [KDR-0036](../kdr/0036-default-function-parameters.md),
case `234`). Arguments are positional, so an omitted argument is always a
trailing one. Generic functions constrain every type parameter with an
interface bound (chapter [08](08-generics.md), `K0801` family, cases
`223`–`233`).

## 3.4 Bindings

`let` binds immutably; assignment to a `let` binding is `K0303` (case `203`).
`mut` declares a mutable binding (case `204`). Bindings are block-scoped;
`let x = expr` infers the binding type locally (case `202`, chapter
[02](02-types.md) §2.5). There is no shadowing rule beyond scope nesting and no
delayed initialization: a binding is initialized where declared.

## 3.5 Error conditions

No codes are introduced here. The governing codes remain: `K0101` (naming),
`K0301` (missing field), `K0302` (missing signature types), `K0303`
(assignment to immutable), `K0801`-family (chapter 08 bounds), `K0906`/`K0907`
(rejected declaration forms).

## 3.6 Conformance cases encoding this chapter

Already passing; this chapter adds no new cases: `002`, `006`–`007` (naming,
no zero values), `201`–`211`, `234` (signatures, bindings, defaults),
`212`–`233` (interface/impl/generic declarations, chapters 07/08),
`301`–`310` (construction and payload round-trips), `906`–`907` (rejected
forms).

## 3.7 Dependencies

- Frozen base: [`keel-core.md`](keel-core.md) §3.
- Decisions: [KDR-0003](../kdr/0003-no-inheritance.md),
  [KDR-0004](../kdr/0004-no-macros.md),
  [KDR-0006](../kdr/0006-no-conditional-compilation.md),
  [KDR-0009](../kdr/0009-no-operator-overloading.md),
  [KDR-0023](../kdr/0023-impls-on-primitive-types.md),
  [KDR-0036](../kdr/0036-default-function-parameters.md).
- Sibling chapters: [02](02-types.md) (the types declarations name),
  [06](06-modules-packages.md) (where declarations live),
  [07](07-interfaces.md), [08](08-generics.md).
