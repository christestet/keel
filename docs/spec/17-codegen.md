# 17 — Schema codegen (`keel gen`)

This chapter is **normative**. It defines `keel gen`, the core-toolchain command
that turns a machine-readable schema into ordinary, typed Keel source, decided in
[`KDR-0104`](../kdr/0104-keel-gen-codegen-surface.md). It does not restate the
frozen rules in [`keel-core.md`](keel-core.md); on any conflict, file an issue
rather than reconciling silently (the prime directive, root
[`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified.** This chapter governs the M7 `keel gen`
work; the first supported format is a **protobuf (`proto3`) subset**, with
OpenAPI and others as additive follow-ons ([`KDR-0104`](../kdr/0104-keel-gen-codegen-surface.md)).

## 17.1 The command

`keel gen <schema>` reads one schema file and writes the equivalent Keel source
to **stdout**. It is an **explicit** command, never an implicit build step — it
is not run during `keel build` ([`KDR-0007`](../kdr/0007-no-build-scripts.md): no
build scripts). Generated files are checked in and reviewed like any other
source.

The format is selected by the schema file's extension: `.proto` selects the
protobuf reader defined here. An unrecognized extension is a usage error (exit
code 2), not a `K####` diagnostic.

## 17.2 Output is ordinary Keel

The generated output is **ordinary Keel source**, not an opaque artifact:

- **Deterministic** — byte-identical for the same input and toolchain version
  (root [`AGENTS.md`](../../AGENTS.md) hard rule 7). Declarations are emitted in
  source order of the schema.
- **Round-trips `keel fmt`** — the generator emits through the same AST
  pretty-printer that backs `keel fmt`
  ([`compiler/keelc-ast`](../../compiler/keelc-ast/src/pretty.rs)), so generated
  source is already canonical: `keel fmt` on the output is a no-op. There is no
  second formatting path (compiler iron rule 3).
- **Carries no generated-code runtime dependency** — output uses only the
  language and standard library; `keel gen` adds no package to the consuming
  project ([`KDR-0104`](../kdr/0104-keel-gen-codegen-surface.md)).
- **Declares its capabilities like any other code** — when a follow-on format
  emits code that touches a capability-bearing surface, that code declares the
  capability in the package manifest ([`11-capabilities.md`](11-capabilities.md));
  nothing is implicit. The `proto3` message subset in §17.3 emits pure data
  types and needs no capability.

## 17.3 The `proto3` message subset

The first reader accepts the message/enum subset of a `proto3` file. The
following productions are recognized; everything else is rejected (§17.5) rather
than silently dropped, so schema drift is visible in review.

Recognized, **ignored** (consumed, no output): a `syntax` statement, a `package`
statement, and `import` statements.

Recognized, **mapped**:

- **`message Name { … }`** → `struct Name { … }`. Message names are
  `UpperCamelCase` in both languages and copy across unchanged.
- A message **field** `[repeated] <type> <name> = <number>;` → a struct field
  `<name>: <keel-type>`. Field names are `snake_case` in both languages and copy
  across unchanged. The field number is consumed and does not appear in the
  output (Keel structs are positional-free).
- **`enum Name { … }`** → `enum Name { … }`, each `NAME = <number>;` becoming a
  payload-free variant. Enum and variant names copy across unchanged.

Scalar type mapping:

| proto3 scalar | Keel type |
|---|---|
| `double`, `float` | `Float` |
| `int32`, `int64`, `uint32`, `uint64`, `sint32`, `sint64`, `fixed32`, `fixed64`, `sfixed32`, `sfixed64` | `Int` |
| `bool` | `Bool` |
| `string` | `String` |

A field whose type names another `message`/`enum` in the file maps to that named
Keel type. A `repeated T` field maps to `List<T>`, where `T` is the mapped
element type.

## 17.4 Deliberately unsupported (rejected, not guessed)

Constructs outside §17.3 are a diagnostic, never a silent approximation:

- `bytes` (Keel Core has no byte-sequence scalar), `map<K, V>`, `oneof`,
  `optional`/`required` presence labels, groups, services/RPCs, options, nested
  messages, and any scalar not in the §17.3 table — each is **`K1602`**.

These are **`K1602`** rather than best-effort mappings because an incorrect type
is worse than an absent one; the reopening clause in
[`KDR-0104`](../kdr/0104-keel-gen-codegen-surface.md) governs widening the
subset.

## 17.5 Error conditions

Registered (append-only) in the `K16xx` band by the implementation PR:

- **`K1601` — malformed schema.** The input is not a well-formed schema in the
  selected format (unterminated `message`, missing field number, stray token).
  Like all readers, `keel gen` never panics on malformed input (compiler iron
  rule 1).
- **`K1602` — unsupported schema construct.** The input is well-formed but uses a
  construct or type outside the supported subset (§17.4).

## 17.6 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `834-gen-proto-struct` | accept (`mode = "gen"`) | a `proto3` file of messages, scalars, a named-type field, and `repeated` generates the expected Keel, and that output is `keel fmt`-idempotent |
| `836-gen-malformed-proto` | reject `K1601` | a `.proto` with a syntactically broken `message` |
| `837-gen-unsupported-type` | reject `K1602` | a field typed `bytes` (a well-formed but unsupported construct) |

## 17.7 Dependencies

- Decision: [`KDR-0104`](../kdr/0104-keel-gen-codegen-surface.md) (in-core,
  stdlib-only, deterministic, `fmt`-clean codegen).
- No build step: [`KDR-0007`](../kdr/0007-no-build-scripts.md) (`keel gen` is
  explicit, never part of `keel build`).
- Capability declaration of emitted code: [`11-capabilities.md`](11-capabilities.md).
- Formatter reuse: the generator emits through the AST pretty-printer in
  [`compiler/keelc-ast`](../../compiler/keelc-ast/src/pretty.rs) (iron rule 3 —
  one formatting path).
- Code registry: `K1601`–`K1602` are registered (append-only) in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  by the implementation PR.
