{% raw %}
# KDR-0014: Brace escaping in string interpolation

- **Status:** accepted
- **Date:** 2026-06-12
- **Scope:** language

## Decision
Inside a string literal, `{expr}` is interpolation; a **literal brace** is
written by doubling: `{{` produces `{` and `}}` produces `}`. No backslash
escape for braces exists. An unmatched single `}` and an unterminated `{…` are
lexical errors (`K0004`, assigned in the lexical spec chapter
[`01-lexical.md`](../spec/01-lexical.md)). This is the only escaping mechanism for braces;
there is one way to write a literal brace.

## Context
The conformance suite proves `"{name}"` interpolation (cases 010, 012, 013) but
is silent on the most common real need: emitting a literal `{` — JSON, format
strings, code generators. With no rule, two implementations will diverge (one
guesses `\{`, another `{{`), and the choice cannot be changed once programs
contain literal braces. Doubling is the established, backslash-free convention
in Rust's `format!`, C#/.NET, and Python f-strings; it keeps Keel strings free
of a general backslash-escape sublanguage at this stage.

## Alternatives considered
- **Backslash escape `\{` / `\}`** (rejected: forces Keel to define a whole
  backslash-escape grammar now, and means every literal backslash before a brace
  becomes ambiguous; doubling is self-contained).
- **No escape; introduce a non-interpolating raw-string form instead**
  (rejected: a second string syntax is more surface than a two-character rule,
  and raw strings are a separate feature decision, not a brace fix).
- **Leave undefined until the lexical spec chapter** (rejected: the suite
  already exercises interpolation; the gap is a live divergence risk, and the
  decision is small and orthogonal).

## Consequences
A literal brace always doubles, including inside an interpolation-free string.
Authors writing JSON-heavy templates type `{{ }}` pairs. Choosing doubling now
forecloses a future backslash-brace meaning; a general escape sublanguage (e.g.
`\n`, `\t`) can still be added later without conflict, since it would not touch
braces. The lexical spec chapter (01-lexical) will encode this with conformance
cases.

## Reopening clause
A measured, recurring readability or correctness bug class caused by doubling in
real Keel code (e.g. brace-dense templates), OR the introduction of a raw-string
literal that makes brace-doubling redundant. Aesthetic preference for backslashes
is not evidence.
{% endraw %}
