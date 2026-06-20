# M6 exit plan

Non-normative roadmap from **green M6 conformance** to the **ROADMAP M6 exit
criterion**. The governing language definition is
[`docs/spec/keel-core.md`](spec/keel-core.md); the executable spec is
[`tests/conformance/`](../tests/conformance/); milestone scope and exit criteria
live in [`ROADMAP.md`](../ROADMAP.md). Decisions are made in
[`docs/kdr/`](kdr/) under the three-PR discipline in
[`AGENTS.md`](../AGENTS.md) (spec → tests → impl, each its own concern).

This note plans work; it does not authorize it. Every step below is still a
KDR → spec → conformance → compiler sequence.

## Status

- M6 stdlib slice complete: `std.time`, `std.json`, `std.http`, `std.sql`,
  `std.config`. See [`docs/milestone-status.md`](milestone-status.md) and the
  per-module notes ([json](m6-json-implementation.md),
  [log](m6-log-implementation.md)).
- Conformance (M6): **163 passed, 0 failed, 3 skipped**. M1–M5 preflight sweep
  green — no earlier gate regressed.
- This is **necessary but not sufficient** for M6 exit.

## Not done yet — the exit gap

ROADMAP M6 **exit** is: [`examples/users-service/main.keel`](../examples/users-service/main.keel)
compiles, runs, and passes its test file. The example is aspirational by
design (see [`examples/CLAUDE.md`](../examples/CLAUDE.md) and the
[service README](../examples/users-service/README.md)) — Core grows to meet it.

Already built and **not** part of the gap: union error returns
(`UserError | sql.Error`), `?` union widening, `catch` + exhaustiveness
(conformance 506/509/510), all five stdlib surfaces, `fn main() -> Result<Unit, E>`
for concrete `E`, primitive `path_param`/`query_param`/`json`.

The gap is five language features plus a test harness:

| # | Feature in the example | Governing decision |
|---|---|---|
| 1 | `fn main() -> Result<Unit, Error>` — universal `Error` | new KDR, extends [0005](kdr/0005-no-exceptions.md) |
| 2 | `Uuid`, `Timestamp`, `Email` scalars + `Uuid.new()`/`Timestamp.now()` | new KDR |
| 3 | `log.info("msg", key: value)` / `http.serve(port:, routes:)` named args | new KDR |
| 4 | `User.from_row(row)` derived associated fn (call **and** `rows.map` value) | amends [0029](kdr/0029-sql-database-access.md) |
| 5 | `catch sql.UniqueViolation` / `sql.NoRows` classification | amends [0029](kdr/0029-sql-database-access.md) |
| 6 | runs on SQLite + passes a test file | tooling (no KDR) |

## Dependency chain

```
1 Error type ─┐
2 scalars ────┼─► example typechecks ─► 4 from_row ─► example lowers ─┐
3 named args ─┘        (4 needs 2 + sql.Row)                          ├─► 6 harness+test ─► M6 EXIT
5 sql.UniqueViolation ───────────────────────────────────────────────┘
```

- 1, 2, 3 are independent and unblock the front-end type surface.
- 4 depends on 2 (scalar fields) and the existing `sql.Row`.
- 5 is small and independent.
- 6 gates on everything: the literal exit criterion.

## The steps (each: KDR → spec → conformance → compiler)

**Step 1 — Universal `Error` type. ✅ DONE** (KDR-0033; spec §5; cases
511/512; impl in `type_absorbs` + catch/match opacity, `K0504`). `Error`
absorbs any propagated error at `?`/return and is opaque (no destructure;
`catch`/`match` on it is `K0504`). M1–M6 preflight green.

**Step 2 — Core scalars `Uuid`/`Timestamp`/`Email`. ✅ DONE** (KDR-0034;
spec §15.34; cases 779–793). `Uuid` and canonicalized `Email` use string-backed
Go values; `Timestamp` uses separate epoch-seconds and nanosecond fields across
the RFC 3339 year range. JSON/HTTP parsing, constructors, canonical
interpolation, offset normalization, named-value JSON writing, and the
three-scalar struct round-trip are implemented. M6 conformance is green.

**Step 3 — Call-site named arguments + structured `log.info`.** General
`name: value` at call sites, plus the structured-log output format. DoD:
`log.info("m", k: v)` output case + a named arg satisfying a `limit: Int = 50`
default.

**Step 4 — Derived `Struct.from_row`.** Auto-derive
`Type.from_row(row) -> Result<Type, sql.Error>` for column-mappable structs,
and allow `Type.from_row` as a first-class value in `rows.map(...)`. Reuses the
existing `Type.method` call path (`Uuid.new`, `Float.from`). **The one novel
piece** — the KDR must pin field→column mapping (by position vs. name), type
coercion, and which `sql.Error` a mismatch raises. DoD: derive a struct from a
row, used both directly and as a mapper.

**Step 5 — `sql.UniqueViolation` classification.** Map driver constraint errors
to a catchable `UniqueViolation` variant (`NoRows` already emitted). DoD:
`catch sql.UniqueViolation` / `sql.NoRows`.

**Step 6 — Example test harness + SQLite validation.** Write the users-service
test file and a runner that builds + runs it on SQLite. Likely **zero dialect
code** — SQLite already accepts `$NNN` params, `RETURNING`, and arbitrary
column type names — but this step is where that assumption is verified. This is
the exit gate.

## Milestone boundary

This is all M6 (the exit criterion itself). It does **not** reach into M7
([`ROADMAP.md`](../ROADMAP.md) "The differentiators"). Scalars (Step 2) are
Core scalar *types*, not the M7 structured-generation work.

## Validation snapshot

Current state, before any step:

```
KEEL_MILESTONE=M6 scripts/preflight.sh   # green: 180 passed, 0 failed, 3 skipped
for m in M1 M2 M3 M4 M5; do KEEL_MILESTONE=$m scripts/preflight.sh; done  # all green
```

Each step lands its own conformance cases and must keep every milestone green.
The final gate (Step 6) additionally runs the example end-to-end on SQLite.

## Next work

Steps 1 and 2 are **done**. Next is **Step 3** (call-site named arguments and
structured `log.info`). See
[`docs/M6-implementation-handoff.md`](M6-implementation-handoff.md) §4 for the
original aspirational-feature list this plan expands.
