# KDR-0035: Multi-line string literals

- **Status:** accepted
- **Date:** 2026-06-20
- **Scope:** language

## Decision

A `"..."` string literal may contain literal newline characters. The lexer no
longer terminates a string at the first newline; a string ends only at an
unescaped closing `"`. A literal newline inside the quotes is part of the
string value, byte-for-byte, exactly as written in the source.

Interpolation is unchanged: an interpolation expression (`{expr}`) still may not
span a newline. A newline encountered while scanning the body of an open `{` is
still `K0004` (unterminated interpolation). Only the surrounding string body may
contain newlines, not the code inside an interpolation hole.

The unterminated-string diagnostic `K0002` is now raised only at end of input
with no closing quote, never at a newline. Escape sequences (`\n`, `\"`, `\\`),
brace doubling (`{{`, `}}`), and interpolation otherwise behave as before.

The formatter renders a multi-line string with its interior newlines intact and
does not re-indent or reflow the string body; `keel fmt` round-trips it. The Go
backend emits the value through Go's escaped double-quote form, so interior
newlines become `\n` in generated source — no behavior change at runtime.

## Context

The M6 exit program (`examples/users-service/main.keel`) writes SQL statements
as multi-line string literals. Under the previous single-line rule the example
does not lex: each query reports `K0002` at its first newline and the trailing
lines parse as stray tokens. Multi-line SQL is the natural way to write the
queries, and forcing single-line strings or `\n`-joined fragments would make the
example unreadable purely to satisfy a lexer limitation.

The single-line restriction was an implementation default, not a designed
constraint: spec §1 describes `String` as `"..."` with interpolation and never
states that a literal may not contain a newline. This KDR makes the spec
explicit and lifts the restriction with the smallest possible change.

## Alternatives considered

- **Keep single-line strings; require `\n` or string concatenation.** Rejected:
  it disfigures the exact program the M6 exit criterion names, for no semantic
  gain.
- **Add a separate triple-quoted or backtick raw-string form.** Rejected: a
  second literal syntax is a new feature the example does not need. Ordinary
  `"..."` spanning lines is sufficient and adds no grammar.
- **Allow interpolation holes to span newlines too.** Rejected: an interpolation
  is code, and a newline inside `{ ... }` almost always signals a missing `}`.
  Keeping `K0004` there preserves good error recovery; the string body gains
  newlines without weakening interpolation diagnostics.

## Consequences

- The lexer accepts newlines in string bodies; `K0002` fires only at end of
  input. A runaway unterminated string now consumes to EOF rather than to the
  next newline, which can widen the reported span but does not change which
  programs are accepted.
- The formatter and Go backend already handle interior newlines (formatter keeps
  them verbatim, backend escapes them), so no second formatting path appears.
- This unblocks the M6 exit example's multi-line SQL. It does not add any new
  literal syntax, so no other milestone scope is touched.

## Reopening clause

Reopen only if corpus evidence shows multi-line string bodies cause a recurring
class of swallowed-to-EOF lexer errors that materially worsen diagnostics across
at least three independent services, and a narrower rule (e.g. requiring an
explicit continuation or a distinct raw-string form) measurably improves error
locality without rejecting the deployed multi-line programs. Preference for
another language's string syntax is not sufficient.
