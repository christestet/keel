# Keel Core v0 — the frozen M0 subset

This document is **normative** for milestones M0–M4. Anything not listed here is
not in Keel Core, even if it appears in `docs/vision.md` or examples. The
conformance suite (`tests/conformance/`) is the executable form of this document;
on conflict, file an issue — do not guess.

## 1. Lexical structure

- Source is UTF-8. Identifiers: `[a-zA-Z_][a-zA-Z0-9_]*`. Type names are
  `UpperCamelCase`, values/functions `snake_case` (enforced: `K0101`).
- Comments: `//` to end of line. No block comments (one way).
- Keywords (reserved): `fn let mut struct enum match if else return use module
  true false test assert catch for in while break continue`
  Reserved for later milestones (cannot be identifiers): `interface scope spawn
  arena extern impl`
- Literals: `Int` (`42`, `1_000`), `Float` (`3.14`), `Bool`, `String` (`"..."`,
  with interpolation `"{expr}"`), `Char` (`'a'`). A `String` literal may span
  multiple lines: literal newlines inside the quotes are part of the value. A
  string ends only at an unescaped closing `"`; reaching end of input first is
  `K0002`. An interpolation hole (`{expr}`) may not span a newline (`K0004`).
- Statement termination: newline-based, like Go. No semicolons (`K0102` if present).

## 2. Types

Primitives: `Int` (64-bit signed), `Float` (64-bit), `Bool`, `String`, `Char`, `Unit`.
Built-in generic types (user-defined generics are NOT in Core):
`Option<T>`, `Result<T, E>`, `List<T>`, `Map<K, V>`.

- **No null.** `Option<T>` with variants `Some(T)` / `None` is the only absence. (`K0201` for any nullish construct)
- **No implicit conversions.** `Int` + `Float` is `K0202`; convert explicitly: `Float.from(i)`.
- Integer overflow panics in debug, wraps with `%+` operators only (explicit) — default `+` panics on overflow in all modes for Core. (`K0203` family)

## 3. Declarations

```keel
struct User {
    id: Int
    name: String
    email: Option<String>
}

enum Status {
    Active,
    Suspended(reason: String),
    Deleted,
}

fn full_name(first: String, last: String) -> String {
    "{first} {last}"
}
```

- **Struct construction requires every field** (`K0301`); no zero values. Field
  defaults: `port: Int = 8080` in the struct declaration are permitted and count
  as provided.
- Function signatures are fully explicit: parameter and return types required
  (`K0302`). `-> Unit` may be omitted. Parameters may declare a default value
  (`limit: Int = 50`) of the parameter's declared type; a call omitting a
  trailing argument receives the default (KDR-0036). Arguments are positional,
  so an omitted argument is always a trailing one.
- `let` is immutable; `mut` declares mutable bindings (`K0303` on assignment to `let`).
- Local type inference only: `let x = expr` infers; signatures never infer.

## 4. Expressions and control flow

Everything is an expression. Block value = last expression. `if/else` is an
expression and both arms must have the same type when the value is used (`K0401`).

`match` is **exhaustive** (`K0402`: missing variant, names the variant). Arms:

```keel
match status {
    Active            => "ok",
    Suspended(reason) => "suspended: {reason}",
    Deleted           => "gone",
}
```

Guards (`Active if user.verified =>`) are in Core. Wildcard `_` is permitted but
lints `K0403` (warning) when matching an enum from the same module — prefer
naming variants so additions break loudly.

Patterns nest and narrow (KDR-0038). A pattern argument may itself be a pattern
(`Err(NotFound(id))`, `Ok(Some(x))`); a bound name carries its payload type into
the arm (`Ok(user)` makes `user` a `User`). A typed binding `x: T` binds and
matches only when the value's runtime type is the union member `T`
(`Err(err: sql.Error)`); `()` matches the unit payload and binds nothing
(`Ok(())`). Exhaustiveness is checked at the outer constructor, not driven into
a union payload; arms are tried top to bottom.

## 5. Errors

- `Result<T, E>` + the `?` operator: `?` unwraps `Ok`/`Some` or returns the
  error/`None` from the enclosing function. Using `?` in a function whose return
  type cannot absorb the error is `K0501` (message must name both types).
- `catch` handles specific variants inline and must be exhaustive over the
  error type or end in a propagating/`other` arm (`K0502`):

```keel
let row = db.get(id) catch err {
    NotFound(_) => return default_user(),
    other       => return Err(other),
}
```

- Union error types in signatures: `-> Result<User, DbError | ParseError>`.
  Matching on a union must cover every member type (`K0503`).
- An **opaque** error type (`sql.Error`, KDR-0037) has no enumerable variants:
  it is *classified*, not destructured, by qualified patterns (`sql.NoRows`,
  `sql.UniqueViolation`). A `catch` over an opaque error needs no exhaustive
  cover and no `other` arm; an error matching no arm **propagates** — it is
  re-wrapped and returned through the enclosing function, whose return type must
  absorb it (an `... | sql.Error` union, or `Error`). This is `catch`'s analogue
  of `?`. The arrow form takes a classification head and binds nothing:
  `... catch sql.NoRows => return Err(NotFound(id))`.
- `Error` is the universal, opaque boundary error type (KDR-0033). Any error
  type coerces into `Error` at `?` and at tail/`return` position in a function
  declaring `-> Result<T, Error>`; `Error` is the only type that absorbs all
  others. `Error` is renderable (string interpolation) but cannot be
  destructured: matching on or `catch`-ing a value of type `Error` is `K0504`.
  Code that must branch on which error occurred declares an explicit union
  instead. `Error` is for boundaries that only propagate (e.g. `main`).
- `panic("msg")` exists for unrecoverable bugs. There is no catch for panics in Core.

## 6. Modules

One module per file. `module name` header optional in single-file programs.
`use std.print` style imports (Core stdlib surface: `print`, `String`, `Int`
methods, `List`/`Map` methods, `checked_div`, `checked_rem` — the real stdlib
is M6).

## 7. Entry point and tests

`fn main() -> Unit` or `fn main() -> Result<Unit, E>` (nonzero exit + error to
stderr on `Err`), where `E` is commonly the universal `Error` (§5) so that any
propagated error coerces in. `test "name" { }` blocks with bare `assert expr` (Core: no
structural diff requirements yet, just pass/fail and the source line).

## 8. Explicitly NOT in Core (compiler must reject, not ignore)

User generics (`K0901`), interfaces (`K0902`), `scope`/`spawn` (`K0903`),
`arena` (`K0904`), `extern`/FFI (`K0905`), attributes/annotations of any kind
(`K0906`), operator overloading (`K0907`), `async`/`await` as identifiers-used-
as-keywords trap (`K0908` with a pointer to [KDR-0002](../kdr/0002-no-async-await.md)).

## 9. Authored spec chapters

| Chapter | Covers |
|---|---|
| [`01-lexical.md`](01-lexical.md) | Brace escaping in string interpolation (`K0004`) — KDR-0014 |
| [`04-expressions.md`](04-expressions.md) | Operator set, precedence, integer division, overflow (`K0202`–`K0204`, `K0003`) — KDR-0013 |

See [`00-spec-plan.md`](00-spec-plan.md) for the full chapter roadmap.
