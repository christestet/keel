# 04 — Expressions and control flow

This chapter is **normative**. It is the literate, testable form of the
expression rules summarised in [`keel-core.md` §4](keel-core.md), and it does
not restate the frozen rules there (everything-is-an-expression, block value,
`if/else` typing `K0401`, `match` exhaustiveness `K0402`/`K0403`). On any
conflict with `keel-core.md`, file an issue rather than reconciling silently
(the prime directive, root [`AGENTS.md`](../../AGENTS.md)).

The new normative material added here is the **built-in operator set and its
semantics**, the decision recorded in
[`KDR-0013`](../kdr/0013-core-operators-and-integer-division.md). No operator is
user-definable (operator overloading remains out of Core, `K0907`).

## 4.1 Operator set

Core defines exactly these built-in operators:

- Arithmetic: `+`, `-`, `*`, `/`, `%` (binary), `-` (unary negation).
- Boolean: `&&`, `||` (binary, short-circuit), `!` (unary).
- Comparison: `<`, `<=`, `>`, `>=`, `==`, `!=`.

Operands must already **share a type**; there are no implicit conversions
(KDR-0009, [`kdr/INDEX.md`](../kdr/INDEX.md)). Mixing types — e.g. `Int + Float` —
is `K0202`; convert explicitly with `Float.from(i)`. `==` and `!=` are defined
for the primitive types in Core (`Int`, `Float`, `Bool`, `String`, `Char`).

## 4.2 Precedence and associativity

Tightest binding to loosest:

| Level | Operators | Associativity |
|---|---|---|
| 1 (tightest) | unary `!`, unary `-` | prefix |
| 2 | `*` `/` `%` | left |
| 3 | binary `+` `-` | left |
| 4 | `<` `<=` `>` `>=` `==` `!=` | **non-associative** |
| 5 | `&&` | left |
| 6 (loosest) | `\|\|` | left |

Comparisons do **not** chain: `a < b < c` is a parse error (`K0003`), never a
silent `(a < b) < c`. Binary arithmetic and the boolean operators are
left-associative.

### 4.2.1 Short-circuit evaluation

`&&` evaluates its right operand only when the left is `true`; `||` evaluates
its right operand only when the left is `false`. This makes the standard guarded
idiom (`if ready && compute()`) safe.

## 4.3 Integer division and remainder

For `Int`:

- `/` **truncates toward zero**: `-7 / 2 == -3`.
- `%` takes the **sign of the dividend**: `-7 % 2 == -1`.

These semantics match the Go backend (KDR-0102), avoiding a lowering mismatch.

### 4.3.1 Division and remainder by zero (`K0204`)

`Int` division or remainder by a zero divisor **panics** (uncatchable, per
KDR-0005), reported under the stable **runtime** code `K0204` (*division or
remainder by zero*). Because the panic is observable only at run time, its
conformance cases are gated to milestone M3 via `case.toml`.

The total, non-panicking alternative is explicit:

```keel
fn safe(a: Int, b: Int) -> Option<Int> {
    checked_div(a, b)   // None when b == 0, otherwise Some(a / b)
}
```

`checked_div(a, b)` and `checked_rem(a, b)` return `Option<Int>` — `None` on a
zero divisor, never null (Core absence doctrine). They are part of the Core
stdlib surface (§6).

### 4.3.2 Overflow

Overflow follows the existing rule (`K0203` family): default `+` and friends
panic on overflow; the `%+` family wraps. `Int.min / -1` overflows and panics
under `K0203` (not `K0204`, since the divisor is non-zero).

## 4.4 Examples (normative, extracted by CI)

```keel
fn main() -> Unit {
    print("{1 + 2 * 3}")        // 7  — * binds tighter than +
    print("{(1 + 2) * 3}")      // 9
    print("{-7 / 2}")           // -3 — truncates toward zero
    print("{-7 % 2}")           // -1 — sign of the dividend
    print("{true && false}")    // false
    print("{!false || false}")  // true
}
```

## 4.5 Conformance cases this chapter introduces

Per the three-PR rule, these land in the following conformance PR (bands `1xx`
arithmetic/boolean values and `4xx` expression control flow, see
[`tests/conformance/README.md`](../../tests/conformance/README.md)):

| Case | Kind | Asserts |
|---|---|---|
| `112-int-subtraction` | accept | `-` on `Int` |
| `113-int-division-truncates` | accept | `-7 / 2 == -3` |
| `114-int-remainder-dividend-sign` | accept | `-7 % 2 == -1` |
| `115-bool-and-short-circuit` | accept | `&&` short-circuits |
| `116-bool-or-short-circuit` | accept | `\|\|` short-circuits |
| `117-bool-not` | accept | unary `!` |
| `118-precedence-mul-over-add` | accept | `1 + 2 * 3 == 7` |
| `119-checked-div-none-on-zero` | accept | `checked_div(a, 0) == None` |
| `120-mixed-operand-type` | reject `K0202` | `Int + Float` without conversion |
| `411-comparison-no-chaining` | reject `K0003` | `a < b < c` is a parse error |
| `121-int-div-by-zero-panics` | reject `K0204`, M3-gated | `a / 0` panics at run time |
| `122-int-rem-by-zero-panics` | reject `K0204`, M3-gated | `a % 0` panics at run time |

## 4.6 Dependencies

- Decision: [`KDR-0013`](../kdr/0013-core-operators-and-integer-division.md).
- Frozen base: [`keel-core.md` §2 (types)](keel-core.md) and
  [§4 (expressions)](keel-core.md).
- Related decisions honoured: KDR-0005 (panics uncatchable), KDR-0009 (no
  implicit conversion / no overloading), KDR-0102 (Go backend semantics). See
  [`kdr/INDEX.md`](../kdr/INDEX.md) for the status of related decisions.
- Code registry: `K0204` registered in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  at spec-writing time ([`docs/spec/AGENTS.md`](AGENTS.md)). `K0202`, `K0203`,
  `K0003` already exist.
