# M6 — `std.json` implementation

Non-normative implementation note. The contract is in
[`docs/spec/15-stdlib-core.md §15.7–15.16`](spec/15-stdlib-core.md) and
[`KDR-0027`](kdr/0027-json-boundary-mapping.md).

## Status

**Done — all 12 conformance cases pass at M6.**

```
KEEL_MILESTONE=M6 scripts/preflight.sh
→ 139 passed, 0 failed, 2 skipped
```

Cases 724–735 exercise every normative requirement in §15.7–15.16: struct
round-trip, write order, missing required fields, absent `Option` fields,
strict and tolerant unknown-field handling, duplicate-key rejection,
integer overflow, non-integer token rejection, enum adjacent-tagged shape,
trailing-input rejection, and the compile-time `K1503` guard.

## What was implemented

**Parser** (`keelc-parse`): a new `parse_call_arg` path recognises
`mode: .tolerant` at call-arg position and encodes it as a sentinel string
literal `"__keel_json_tolerant"` so the KIR lowering layer can tell strict
from tolerant without adding a new AST node. Type arguments on field-call
expressions (`json.parse[T](…)`) are also gated to milestone ≥5 here.

**Formatter** (`keelc-ast/pretty.rs`): the pretty-printer detects the sentinel
literal when the callee is `json.parse` and round-trips it as `mode: .tolerant`,
so `keel fmt` is stable across parse–print cycles.

**Typechecker** (`keelc-resolve`): `json.parse[T]` and `json.write(value)`
both call `ctx.is_json_representable` on the target type. Any non-representable
type (e.g. `Unit`, function values, unconstrained type parameters) emits
`K1503` at the type-argument or value-expression span.

**KIR lowering** (`keelc-kir/lower.rs`): the `mode: .tolerant` sentinel travels
into the KIR call args unchanged; the backend inspects it there.

**Go backend** (`keelc-backend-go`): emits statically-derived codecs:
- `keelJSONParse_T` / `keelJSONDecode_T` — typed decoders per target type
- `keelJSONEncode_T` — typed encoders per source type
- Struct decoders check for duplicate fields (detected in the token stream before
  conversion), missing required fields, optional-field defaulting to `None`, and
  unknown-field handling gated on the `tolerant bool` parameter.
- Enum decoders enforce the adjacent-tagged shape `{"variant":"…","fields":{…}}`.
- The encoder writes struct fields in declaration order and enum fields as
  `variant` then `fields`, satisfying the determinism requirement in §15.12.
- `keelJSONParseRaw` checks for trailing non-whitespace after the root value.
- Integer decoding rejects decimal points and exponents (`strconv.ParseInt`
  after a pre-check for `.`/`e`/`E`); float decoding rejects non-finite results.

**Types** (`keelc-types/infer.rs`): `is_json_representable` implements the
recursive representability check from §15.8. `Unit`, function values,
`TypeParam`, and `Unknown` return `false`; all listed representable types
return `true` when their inner types are representable.

**Diagnostic registry** (`keelc-diag/registry.rs`): `K1503 — unsupported JSON
target` was pre-registered in the M6 spec PR. No new codes in this slice.

## What is NOT done (still M6)

- `std.http`, `std.sql`, `std.log`, `std.config` — not started. The spec
  chapter (`15`) currently covers only `std.time` and `std.json`.
- `Map<String, T>` codec — representability check passes, but the backend codec
  emitter hits the unsupported branch. Blocked until a conformance case is
  written first (per hard rule 8).
- `json.Error` as a catchable Keel enum in user code — the error variants
  (`Syntax`, `TypeMismatch`, etc.) are represented as `KeelEnum` tagged values;
  catch arms match on the tag string exactly as specified, but the Keel-level
  enum type `json.Error` is not declared in source and cannot be named in a type
  annotation today.
- `keel gen` schema metadata / wire-name overrides — outside M6 scope per KDR-0027.
- `examples/users-service/main.keel` M6 exit criterion — requires `std.http`
  and likely `std.sql` as well.

## Lessons from prior art applied here

Go's `encoding/json` v1 accepts duplicate keys and decodes numbers via
`float64`; KDR-0027 explicitly rejects both. The duplicate-key check runs on
the token stream (inside `keelJSONRead`) so the backend never inherits Go's
last-key-wins map behaviour. Integer decoding goes direct to `strconv.ParseInt`
after rejecting tokens containing `.`, `e`, or `E`, avoiding the precision
loss of the float-then-truncate approach used by `json.Decoder.Decode` on
`interface{}`.

Serde's attribute-driven rename/tag/default system was considered and rejected
(KDR-0004, KDR-0027). The current codec path has no user-visible configuration
hooks, which means it can be replaced in full when the native backend arrives —
the conformance suite is the equivalence proof.

## Dependency chain

| Document | Why it matters |
|---|---|
| [`docs/spec/15-stdlib-core.md`](spec/15-stdlib-core.md) | Normative surface and error codes |
| [`KDR-0027`](kdr/0027-json-boundary-mapping.md) | Wire semantics, rejected alternatives |
| [`KDR-0015`](kdr/0015-boundary-doctrine.md) | Typed boundaries, strict default, tolerant mode |
| [`KDR-0004`](kdr/0004-no-macros.md) | No annotations, no user-defined codec hooks |
| [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) | Pipeline, KIR contract, backend gotchas |
| [`tests/conformance/724-*` – `735-*`](../tests/conformance/) | Executable definition of correct |

## Milestone boundary

The next M6 module is `std.http`. Follow the same three-PR sequence: KDR →
spec (§15.x) + conformance cases → implementation. No implementation work
begins until a KDR is accepted and conformance cases are merged.

`examples/users-service/main.keel` is the M6 exit criterion (see
[`ROADMAP.md`](../ROADMAP.md)); it is not met until at minimum `std.http` lands.
