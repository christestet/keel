# KDR-0034: Core boundary scalars

- **Status:** proposed
- **Date:** 2026-06-20
- **Scope:** language

## Decision

Keel has three compiler-known, opaque boundary scalar types: `Uuid`,
`Timestamp`, and `Email`. They are value types, support `==` and `!=`, and are
renderable by string interpolation. They do not implicitly convert to or from
`String`, expose fields, or permit user-defined construction.

Their initial constructors and canonical text forms are:

- `Uuid.new() -> Uuid` creates a random version-4 UUID. Its canonical text is
  lower-case hexadecimal in the `8-4-4-4-12` form. Parsing accepts exactly that
  form and requires the RFC 4122 variant and version 4.
- `Timestamp.now() -> Timestamp` reads the current UTC wall clock. A timestamp
  is an instant stored as signed nanoseconds from the Unix epoch. Its canonical
  text is UTC RFC 3339 with `Z`; fractional seconds are omitted when zero and
  otherwise emitted to the necessary nanosecond precision with trailing zeroes
  removed. Parsing accepts exactly the canonical form and rejects values outside
  the signed 64-bit nanosecond range.
- `Email` is an ASCII address with exactly one `@`. The local part is one or
  more dot-separated atoms containing letters, digits, or
  ``!#$%&'*+-/=?^_`{|}~``; it is at most 64 bytes. The domain is one or more
  dot-separated labels containing letters, digits, or interior `-`; each label
  is at most 63 bytes and the domain is at most 253 bytes. The whole address is
  at most 254 bytes. Its canonical text is the input text unchanged. This is a
  deliberately narrow address syntax, not an RFC 5322 mailbox parser: comments,
  display names, quoted local parts, internationalized addresses, and address
  literals are rejected.

All three map to JSON strings. `json.parse<T>` validates the canonical form and
returns `json.TypeMismatch` for an invalid scalar string; `json.write` emits the
canonical form. `http.Request.path_param<T>` and `query_param<T>` use the same
parsers. `Uuid.new()` and `Timestamp.now()` are the only source constructors in
the initial surface; `Email` values enter through typed boundaries such as JSON
and, once specified, SQL row mapping.

SQL parameter and row mappings for these types are fixed by the `std.sql`
specification and KDR-0029's follow-up decision, not by this KDR.

## Context

The M6 exit program uses UUIDs as identifiers, timestamps as creation instants,
and email addresses as validated boundary values. Representing all three as
`String` would let malformed values escape the parse boundary and would make
function signatures lie about accepted data. User-defined wrappers cannot fill
the gap: Keel has no user-defined constructors, custom JSON codecs, reflection,
or annotations, by design.

KDR-0015 requires external data to become honest internal types at explicit
parse points. KDR-0027 already reserves JSON support for compiler-known standard
boundary scalars. This decision supplies the smallest scalar set required by
the M6 example and pins backend-independent text behavior before the compiler
implements it.

The narrow `Email` grammar is intentional. Full RFC 5322 mailbox syntax includes
display names, comments, quoting, domain literals, and obsolete forms that are
not useful as service identity values. Accepting those forms would create a
large parser and still would not prove that a mailbox exists or can receive
mail.

## Alternatives considered

- **Use `String` aliases.** Rejected: aliases do not make invalid states
  unrepresentable and cannot require validation at JSON or HTTP boundaries.
- **Add a general user-defined newtype or scalar protocol.** Rejected: M6 needs
  three values, not a new abstraction mechanism. Custom codecs would also
  conflict with KDR-0004 and KDR-0027's single wire mapping.
- **Put the types in separate stdlib modules.** Rejected: the example needs the
  types in signatures without imports, and these types participate directly in
  compiler-derived JSON and HTTP parsing. They are primitive boundary values,
  not service APIs.
- **Store `Timestamp` as text.** Rejected: textual storage makes ordering and
  equality depend on equivalent spellings. A UTC nanosecond instant has one
  value and one canonical rendering.
- **Implement full RFC 5322 email syntax.** Rejected: it adds substantial parser
  complexity for forms unsuitable for ordinary backend identity fields, while
  still providing no deliverability guarantee.
- **Expose `parse(String)` constructors.** Rejected for the initial surface:
  the M6 program receives these values only from existing typed boundaries.
  Adding a second parsing API before a program needs it would duplicate boundary
  behavior.

## Consequences

- The compiler, formatter, type checker, KIR, and backend gain three closed
  scalar types rather than a generic custom-scalar mechanism.
- JSON and HTTP parsing share one strict parser per scalar, so the same text
  cannot be accepted at one boundary and rejected at another.
- `Uuid.new()` and `Timestamp.now()` are nondeterministic runtime operations;
  compiler output and serialization remain deterministic.
- `Email` intentionally rejects some standards-valid mailbox spellings. Systems
  that must preserve arbitrary mailbox syntax use `String` at that boundary.
- SQL row decoding must validate scalar values rather than silently wrapping
  malformed database data; the exact column mapping remains Step 4 work.

## Reopening clause

Reopen a scalar's accepted syntax only with corpus evidence that at least three
independent Keel services must accept a currently rejected deployed wire form,
plus a deterministic canonicalization rule that preserves strict boundary
parsing. Reopen the closed scalar set when a fourth domain scalar recurs across
at least three independent services and cannot be represented honestly by an
existing type or schema-generated boundary type. Preference for a generic
newtype facility or another language's standard library is not sufficient.
