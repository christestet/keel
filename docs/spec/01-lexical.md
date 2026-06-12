# 01 — Lexical structure

This chapter is **normative**. It is the literate, testable form of the lexical
rules summarised in [`keel-core.md` §1](keel-core.md). It does **not** restate
the frozen rules there (UTF-8 source, identifier grammar, comments, keyword set,
literal forms, newline-based termination); on any conflict between this chapter
and `keel-core.md`, file an issue rather than reconciling silently (the prime
directive in the root [`AGENTS.md`](../../AGENTS.md)).

The only new normative material added here is **brace escaping inside string
literals**, the decision recorded in
[`KDR-0014`](../kdr/0014-interpolation-brace-escaping.md).

## 1.1 String literals and interpolation

A `String` literal is delimited by `"`. Inside it:

- `{expr}` is an **interpolation**: the text between the braces is a Keel
  expression whose value is converted to `String` and spliced in. This is the
  behaviour already proven by conformance cases `010`, `012`, `013`.
- A **literal brace** is written by **doubling**:
  - `{{` produces a single `{`.
  - `}}` produces a single `}`.

  Doubling is the *only* way to write a literal brace. There is no backslash
  escape for braces, and a literal backslash before a brace has no special
  meaning. This holds in every string literal, including ones that contain no
  interpolation.

### 1.1.1 Examples (normative, extracted by CI)

```keel
fn main() -> Unit {
    print("{{")        // prints: {
    print("}}")        // prints: }
    let name = "keel"
    print("{{{name}}}") // prints: {keel}
}
```

In `"{{{name}}}"` the leading `{{` is a literal `{`, `{name}` is the
interpolation, and the trailing `}}` is a literal `}`.

## 1.2 Malformed interpolation (`K0004`)

The following are **lexical errors**, reported under the stable code `K0004`
(*malformed string interpolation*):

- An **unmatched single `}`** inside a string literal — a `}` that is neither
  part of a `}}` pair nor the close of an interpolation. Example: `"a } b"`.
- An **unterminated interpolation** — a `{` that opens an interpolation which
  the string literal ends before closing. Example: `"hello {name`.

A `{` always begins an interpolation unless it is part of a `{{` pair; therefore
a literal `{` that is not doubled and not closed is an unterminated
interpolation, not an unrecognised character.

`K0004` is distinct from `K0002` (*unterminated string literal*, the missing
closing `"`) and from `K0001` (*unrecognized character*). When both an
unterminated interpolation and a missing closing quote are present, the
interpolation error (`K0004`) is the reported diagnostic, because the unbalanced
brace is the more specific cause.

## 1.3 Conformance cases this chapter introduces

Per the three-PR rule, these cases land in the following conformance PR (band
`0xx`, see [`tests/conformance/README.md`](../../tests/conformance/README.md)):

| Case | Kind | Asserts |
|---|---|---|
| `014-brace-escape-literal` | accept | `"{{"`→`{`, `"}}"`→`}` in interpolation-free strings |
| `015-brace-escape-with-interpolation` | accept | `"{{{name}}}"` mixes literal braces and `{name}` |
| `016-lone-close-brace` | reject `K0004` | `"a } b"` — unmatched single `}` |
| `017-unterminated-interpolation` | reject `K0004` | `"hello {name` — `{` opened, never closed |

## 1.4 Dependencies

- Decision: [`KDR-0014`](../kdr/0014-interpolation-brace-escaping.md).
- Frozen base: [`keel-core.md` §1](keel-core.md).
- Code registry: `K0004` registered in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  at spec-writing time (spec discipline, [`docs/spec/AGENTS.md`](AGENTS.md)).
