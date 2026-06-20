# KDR-0032: Type parameters and arguments use `<T>`

- **Status:** proposed
- **Date:** 2026-06-20
- **Scope:** language

## Decision

Type parameters and type arguments are written with angle brackets, `<T>`,
in **every** position — there is no other syntax:

- **Declaration sites** — `fn greet<T: Stringer>(value: T)`, `struct Box<T>`,
  `enum Tree<T>`, `impl Stringer for List<T>`.
- **Call sites** — `json.parse<User>(body)`, `config.load<AppConfig>()`,
  `req.path_param<Uuid>("id")`, `greet<Int>(42)`.
- **Type positions** — already `<T>` (`Option<T>`, `Result<T, E>`, `Map<K, V>`).

The bracket form `[T]` (`fn greet[T]`, `json.parse[User]`) is removed from the
language and the compiler. No alias or deprecation period is kept.

Declaration sites are unambiguous: `<` following a declared name always opens a
type-parameter list. A **call-site** type-argument list is recognized when a name
or field access is immediately followed by `<`, the contents parse as a
comma-separated type list, and the closing `>` is immediately followed by `(`. In
any other expression position `<` remains the less-than operator.

## Context

Keel already writes type arguments with angle brackets everywhere a *type*
appears: `Option<T>`, `Result<T, E>`, `List<T>`, `Map<K, V>` (keel-core §). The
design vision writes boundary parse points the same way — `json.parse<T>`,
`proto.decode<T>` (vision §6) — and the canonical M6 service
(`examples/users-service/main.keel`) uses `<T>` at every call site.

The implemented compiler had drifted to a second bracket style, `[T]`, for
type *parameters* (`fn greet[T: Stringer]`) and type *arguments*
(`json.parse[User]`, `greet[Int](42)`). That left two different brackets for one
concept — a type — which is exactly the cognitive-overload cost that the "one way
to do things" principle (KDR-0001) exists to prevent. A reader seeing `Option<T>`
in a signature and `json.parse[T]` at the call had to hold two spellings of the
same idea. Fixing only the call site would not remove the overload — it would move
it, leaving `fn greet[T]` declaring what `greet<Int>()` then supplies. The bracket
must go everywhere or nowhere; the example and vision say everywhere.

The historical reason languages avoid `<T>` at call sites is the ambiguity with
the comparison operator (`a < b > c`). That ambiguity does not exist in Keel:
chained comparison is already a compile error (`K0003`, "comparison operators
cannot be chained"). With chaining banned and user-defined generics absent from
Core (`K0901`), the sequence `name < TypeList > (` has no competing legal parse,
so the disambiguation is a rule the language already enforces for an unrelated
reason — it costs no new grammar machinery and no new error.

## Alternatives considered

- **Keep `[T]` anywhere (call sites, or declaration sites only).** Rejected: two
  bracket styles for one concept, contradicting KDR-0001 and diverging from the
  vision and the example. The only thing `[T]` bought was dodging an ambiguity
  that Keel does not have — and it only ever applied to the call site, not the
  unambiguous declaration site.

- **Support both `[T]` and `<T>` as aliases during a transition.** Rejected:
  an alias *is* the two-spellings problem the decision exists to remove; it
  normalizes the drift instead of ending it. The migration surface is tiny
  (the `json.parse` cases) and mechanical.

- **Rust-style turbofish `::<T>`.** Rejected: Keel has no `::` path separator,
  and the extra punctuation is unjustified once chained comparison is banned.

## Consequences

- The parser recognizes call-site type arguments only in the unambiguous
  `name<…>(` / `field<…>(` shape; `<` elsewhere is unaffected. Speculative parse
  with restore on failure keeps the comparison path intact.
- All existing `json.parse[T]` source, spec text, and conformance cases migrate
  to `<T>`. There is exactly one type-argument syntax in the language after this.
- A future call site that wants type arguments *without* an immediately following
  `(` (e.g. a value-returning generic with no call) is not expressible by this
  rule and would require a follow-up decision; no such site exists in Core today.

## Reopening clause

Corpus evidence that the `name<…>(` recognition rule forces an unnatural
rewrite for a real, non-call construct that genuinely needs explicit type
arguments, AND that no extension of the rule (e.g. allowing a following `{` or
end-of-expression) can disambiguate it from comparison without reintroducing the
chained-comparison ambiguity that `K0003` forbids.
