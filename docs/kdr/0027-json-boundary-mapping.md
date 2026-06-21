# KDR-0027: Typed JSON boundary mapping

- **Status:** accepted
- **Date:** 2026-06-19
- **Scope:** stdlib

## Decision

`std.json` is a compiler-known, typed boundary. Its two operations are
`json.parse<T>(input)` and `json.write<T>(value)`. Both return `Result`; malformed
input and values that JSON cannot represent are ordinary `json.Error` values,
never panics. The exact spelling of the tolerant argument remains the spelling
already fixed by [KDR-0015](0015-boundary-doctrine.md); this KDR fixes the wire
semantics beneath it.

The initial JSON surface has one mapping and no user-configurable dialect:

1. `T` must be concrete and JSON-representable at the call site. The compiler
   derives the codec statically; there is no reflection, serializer interface,
   annotation, or runtime type registry. The initial representable set is
   `Bool`, `String`, `Char`, `Int`, `Float`, `Option<T>`, `List<T>`,
   `Map<String, T>`, structs, enums, and compiler-known standard boundary scalar
   types. Other map key types and unconstrained generic values are compile-time
   errors. A dynamic `json.Value` tree is not part of the initial surface.

2. JSON object names match hand-written struct field names exactly and
   case-sensitively. A missing field is an error unless its Keel type is
   `Option<T>`, in which case it becomes `None`. JSON `null` is accepted only for
   `Option<T>` and also becomes `None`; a present non-null value becomes
   `Some(value)`. A source-level struct default is a construction convenience,
   not a wire default, and is never silently applied while parsing.

3. Generated boundary types may carry compiler-owned wire-name and schema
   metadata from `keel gen`. That metadata is not expressible as a user
   annotation and cannot create a repository-local serialization dialect.
   Hand-written types have no rename escape hatch: either use the wire name or
   generate the boundary type.

4. Enums written without schema metadata use one uniform adjacent-tagged shape:
   `{"variant":"Active","fields":{}}` for a unit variant and
   `{"variant":"Suspended","fields":{"reason":"maintenance"}}` for a
   payload variant. Both keys are required, no other keys are accepted in strict
   mode, and payload names match their Keel names exactly. Untagged, externally
   tagged, and trial-in-declaration-order decoding are not supported. Generated
   schema metadata may select the representation required by that schema.

5. Numbers are checked, not coerced. `Int` accepts only a JSON integer token
   with no fraction or exponent and a value in Keel's signed 64-bit range.
   `Float` accepts any finite JSON number representable by Keel `Float`.
   Strings and booleans never coerce to numbers, and non-finite float values are
   rejected by `json.write`.

6. Strict parsing rejects unknown fields, duplicate object names, invalid UTF-8,
   invalid escapes, trailing non-whitespace input, type mismatches, and numeric
   overflow. Tolerant mode changes exactly one rule: unknown fields are ignored
   and produce the `schema_drift` event required by KDR-0015. It does not forgive
   duplicates, malformed text, wrong types, missing fields, or overflow.

7. Writing is byte-deterministic. It emits valid UTF-8, no insignificant
   whitespace, struct fields in declaration order, enum keys in the order shown
   above, and map keys in lexicographic Unicode scalar-value order. Integers use
   canonical decimal notation; finite floats use the shortest representation
   that round-trips to the same value. Implementations may not expose backend
   map iteration order or backend-specific HTML escaping.

`json.Error` has stable semantic categories for syntax (with byte offset), type
mismatch (with value path and expected type), missing field, unknown field,
duplicate field, numeric range, and non-finite output. The stdlib specification
fixes their exact enum payloads and diagnostic codes before implementation; it
may refine messages but may not collapse these categories.

## Context

[KDR-0015](0015-boundary-doctrine.md) already decides that JSON parsing is typed,
strict by default, and explicitly tolerant only for unknown fields. What remains
is the set of choices that otherwise become accidental backend behavior:
duplicate names, UTF-8 handling, number coercion, enum shape, absent fields, and
output order.

The useful lessons from existing implementations are mostly their compatibility
costs. Go's original `encoding/json` accepts duplicate names, replaces invalid
UTF-8, loosely matches field names, ignores unknown fields, and decodes dynamic
numbers through floating point. Its v2 API reverses several of those defaults,
which is evidence that permissiveness is easy to add and hard to remove. Python's
standard JSON decoder likewise accepts repeated names and non-standard
`NaN`/`Infinity` by default. Serde provides strong typed decoding, but its
attribute-driven rename and enum configuration creates many wire dialects, and
untagged enums try alternatives in order. Those are reasonable compatibility
tools for established languages; they conflict with Keel's one-language shape,
[KDR-0004](0004-no-macros.md), and deterministic boundary doctrine.

The adopted design takes typed derivation and explicit schema generation, while
rejecting permissive historical defaults and per-type configuration. It also
keeps the initial implementation small: one compiler-derived codec path rather
than a reflection API plus a configurable serializer framework.

## Alternatives considered

- **Use the Go backend's JSON defaults.** Rejected: backend defaults are
  intentionally compatibility-oriented and violate Keel's strictness,
  integer-precision, UTF-8, case-sensitivity, and determinism requirements.
- **Serde-style user attributes for rename, defaults, flattening, and enum
  representation.** Rejected: KDR-0004 prohibits annotations, and combinations
  of options create local wire dialects. Schema-driven code generation is the
  sanctioned compatibility mechanism.
- **Parse first to a dynamic JSON tree.** Rejected for the initial surface: it
  turns boundary validation into scattered field access and contradicts
  parse-don't-validate. A dynamic tree can be reconsidered for demonstrated
  proxy or document-processing workloads.
- **Last duplicate key wins.** Rejected: it hides producer bugs, enables parser
  differential attacks, and makes the result depend on a condition ordinary
  object types cannot represent.
- **Decode all numbers as `Float` and convert later.** Rejected: integers above
  the exact floating-point range silently lose information. The target type is
  known, so conversion must be direct and checked.
- **Use untagged enums and try variants in declaration order.** Rejected:
  overlapping variants are ambiguous and a harmless declaration reorder can
  change runtime behavior.
- **Make `json.write` infallible.** Rejected: Keel `Float` can contain a
  non-finite value that JSON cannot represent. Returning `Result` avoids both
  invalid output and a panic.

## Consequences

- JSON behavior is independent of the Go backend and remains portable to the
  planned native backend.
- Boundary failures are explicit and testable; tolerant mode stays narrow and
  observable.
- Hand-written integration with wire names that are not legal or desirable Keel
  names requires generated boundary types. This is deliberate pressure toward
  schema-owned APIs, but it is less convenient for ad-hoc third-party JSON.
- Adding a struct field changes its encoded form and makes it required when
  decoding unless declared `Option<T>`; source defaults do not mask schema
  drift.
- The users-service example must handle `json.write` failure explicitly when
  the stdlib slice lands.
- Conformance must cover every rejection above, tolerant-mode observability,
  integer boundaries, enum ambiguity avoidance, and byte-identical output.

## Reopening clause  *(required)*

Reopen only with corpus evidence from real Keel programs showing either:

1. a material class of proxy, patch, or document-processing programs cannot be
   expressed without a dynamic JSON tree and currently resort to unsafe string
   manipulation or FFI; or
2. schema generation cannot represent a common deployed JSON convention, with
   measured frequency and concrete schemas, and the proposed extension preserves
   strict parsing, determinism, and the prohibition on user-defined annotation
   dialects.

Popularity of another serializer, preference for its syntax, or requests for a
generic configuration hook are not sufficient.
