# KDR-0025: Type-driven structured generation and compile-time schema derivation

- **Status:** proposed
- **Date:** 2026-06-18
- **Scope:** language

## Decision

The compiler may **derive, at compile time, a serialization schema** (JSON Schema
and/or a constrained-decoding grammar) from any Keel type `T` that participates in
boundary parsing, and expose it both to `keel gen` and to a structured-generation
surface. Derivation is a **compile-time facility** — the compiler already knows
every struct field, enum variant, and `Option` — emitted through codegen or a
compiler built-in, with **no runtime reflection** ([`KDR-0004`](0004-no-macros.md)
is preserved). The surface

```keel
generate<T>(model, prompt) -> Result<T, ModelError>
```

constrains and parses model output into a `T`, applying the boundary doctrine
([`KDR-0015`](0015-boundary-doctrine.md)): **strict by default** (unknown fields
are errors), with an explicit `mode: .tolerant` relaxation that ignores unknown
fields and logs a structured `schema_drift` event. The same derived schema is the
single source of truth for an agent tool's function-calling definition
([`KDR-0024`](0024-ai-infrastructure-and-agent-positioning.md)).

## Context

Structured output / function-calling is *the* integration primitive between typed
programs and LLMs: the model must emit data conforming to a schema, and the
program must parse it safely. Today this is done by hand-writing JSON Schemas (or
Pydantic/zod models) alongside the language types — two sources of truth that
drift, producing exactly the boundary bug class [`KDR-0015`](0015-boundary-doctrine.md)
exists to eliminate. An LLM is the ultimate untrusted boundary; "parse, don't
validate" *is* structured generation.

The tension is real: deriving a schema from a type resembles reflection, which
Keel rejects ([`KDR-0004`](0004-no-macros.md)). The resolution is that derivation
happens at **compile time**, the same way the Go backend already enumerates struct
fields during code generation — it is not a runtime `reflect`-style facility, and
it is not user-level metaprogramming. This mirrors how boundary parsers
(`json.parse<T>`, `proto.decode<T>`, vision §6) are already expected to be
type-directed.

## Alternatives considered

- **Runtime reflection** (inspect `T` at runtime to build the schema). Rejected:
  squarely violates [`KDR-0004`](0004-no-macros.md) (no reflection); also a
  startup-cost and opacity tax.
- **Hand-written schemas alongside types.** Rejected: two sources of truth that
  drift — the precise failure [`KDR-0015`](0015-boundary-doctrine.md) was written
  to prevent.
- **User-level macros / `derive` attributes** (Rust-style). Rejected: Keel has no
  macros or metaprogramming ([`KDR-0004`](0004-no-macros.md)); derivation must be
  a compiler/codegen facility, not user syntax.
- **No structured-generation surface; make users parse raw strings.** Rejected:
  cedes the core genAI integration primitive and pushes the untrusted-boundary
  burden onto every caller, against [`KDR-0015`](0015-boundary-doctrine.md).

## Consequences

- Tool schemas and structured outputs derive from Keel types — one source of
  truth; `keel audit`/`keel gen` can surface the exact schema a model is asked to
  produce.
- Requires a **stable, specified type→schema mapping** (primitives, `struct`,
  `enum` with payloads, `Option` → optional/nullable, `List`/`Map`, and bounded
  generics from [`KDR-0022`](0022-interface-constrained-generics.md)), each entry
  testable, with conformance cases — landed via the normal spec→tests→impl
  sequence (root `AGENTS.md` hard rule 1).
- Honest optionality carries through: a field the schema cannot guarantee is
  `Option<T>`, exactly as [`KDR-0015`](0015-boundary-doctrine.md) requires for any
  wire data.
- Depends on [`KDR-0024`](0024-ai-infrastructure-and-agent-positioning.md) being
  accepted (it is the positioning that motivates this surface) and is post-M6
  (needs `std.json` and the schema machinery).
- Constrained-decoding grammars are emitted for backends that support them
  (e.g. GBNF via a capability-governed FFI); where unsupported, `generate<T>`
  degrades to generate-then-parse with the same strict/tolerant semantics.

## Reopening clause  *(required)*

Reopen if corpus evidence from at least three distinct Keel codebases each
exceeding 10,000 lines shows that the **compile-time-only, no-reflection**
constraint blocks a common, real structured-generation need — a schema shape that
cannot be expressed by the type→schema mapping and cannot be served by `keel gen`,
a build-time step, or a concrete-type workaround — such that teams are driven to
hand-write and hand-maintain divergent schemas anyway. Demonstrated developer
demand measured in corpus frequency, not preference or "other frameworks expose
reflection," is the bar.
