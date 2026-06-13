# KDR-0013: Core operator set and integer division semantics

- **Status:** accepted
- **Date:** 2026-06-12
- **Scope:** language

## Decision
Keel Core gains the boolean operators `&&`, `||`, `!` with **short-circuit**
evaluation, and the arithmetic operators `-`, `/`, `%` (joining the existing
`+`, `*`). Operands must already share a type (no implicit conversion, KDR-0009,
[`INDEX.md`](INDEX.md); `K0202` otherwise). For `Int`:

- Division `/` truncates toward zero; remainder `%` takes the sign of the
  dividend (`-7 / 2 == -3`, `-7 % 2 == -1`).
- Division or remainder by zero **panics** (uncatchable, per KDR-0005, [`INDEX.md`](INDEX.md)),
  reported under the stable runtime code `K0204`. The total, non-panicking
  alternative is explicit: `checked_div(a, b)` and `checked_rem(a, b)` return
  `Option<Int>` (`None` on a zero divisor) — absence is modeled with `Option`,
  never null (KDR core doctrine).
- Overflow follows the existing rule (`+` etc. panic; `%+` family wraps),
  already coded `K0203`; `/` of `Int.min / -1` overflows and panics under
  `K0203`.

Precedence, tightest to loosest: unary `!` and unary `-`; then `* / %`; then
binary `+ -`; then comparisons `< <= > >= == !=`; then `&&`; then `||`.
Comparisons are non-associative (no chaining `a < b < c`); binary arithmetic and
boolean operators are left-associative. `==`/`!=` are defined for the primitive
types already in Core.

## Context
Core today proves only `+`, `*`, `>`, `<`, `==` through the conformance suite,
so no realistic program is expressible: there is no way to write `if a && b`, to
subtract, or to divide. Every general-purpose language needs these; the open
questions are not *whether* but *what the edges do*. Division by zero and signed
integer division/remainder are the classic sources of silent cross-language
divergence (C leaves both undefined; Python floors; Go truncates). Pinning them
now, while the corpus is empty, costs nothing; pinning them after programs exist
is a breaking change. Short-circuit `&&`/`||` is near-universal precisely
because eager boolean evaluation surprises readers and forces guarding against
side effects.

## Alternatives considered
- **Division by zero returns `Result`/`Option` for the bare `/` operator**
  (rejected: makes the common, provably-safe case syntactically heavy — every
  `/` would need `?`/`match` — while the bug case is exactly what KDR-0005 says
  ([`INDEX.md`](INDEX.md))
  should panic; the `checked_*` functions give the total form to those who want
  it, keeping the operator clean).
- **Floor division + divisor-signed modulo (Python model)** (rejected: Keel's
  audience is backend engineers fluent in Go/Rust/C semantics; matching the
  runtime backend (Go, KDR-0102) also avoids a lowering mismatch).
- **Eager (non-short-circuit) boolean operators** (rejected: surprising, and
  prevents the standard `if ptr_ok && deref()` guarding idiom).
- **Defer `/` `%` to a later milestone** (rejected: blocks writing arithmetic
  programs at all; the edge decisions do not get easier later).

## Consequences
Makes ordinary arithmetic and boolean logic expressible and unambiguous across
backends. Forces a defined runtime panic path (`K0204`) and its `checked_*`
escape into the Core stdlib surface (§6). Comparison non-chaining means `a < b <
c` is a parse error, not a silent `(a < b) < c`. No operator is user-definable
(KDR-0009 stands, [`INDEX.md`](INDEX.md)); this KDR only fixes the *built-in* set and its semantics.

## Reopening clause
Corpus evidence that truncating division or dividend-signed remainder is the
measured source of a recurring bug class in real Keel code, OR a backend port
where these semantics cannot be lowered without per-operation runtime cost.
Operator-syntax preference and "language X chose floor division" are not
evidence.
