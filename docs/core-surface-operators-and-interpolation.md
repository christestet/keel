# Core surface growth: operators + interpolation brace escaping

Non-normative handoff note. The governing language definition remains
[`docs/spec/keel-core.md`](spec/keel-core.md); the executable spec remains
[`tests/conformance/`](../tests/conformance/). This note records two accepted
language decisions and the implementation pipeline they unblock, so a fresh
session can continue without re-deriving context.

## Status

Done this session:

| Area | State |
|---|---|
| Conformance suite | Grown to 74 structurally-valid cases (committed `cde20b8 tests: extend conformance test set`, on `origin/main`). All additions use already-proven Core syntax only. |
| [`KDR-0013`](kdr/0013-core-operators-and-integer-division.md) | **accepted** — boolean `&& \|\| !` (short-circuit), arithmetic `- / %`, truncate-toward-zero division, dividend-signed remainder, div/rem-by-zero **panics** under new code `K0204`, total escape `checked_div`/`checked_rem -> Option<Int>`. Precedence/associativity fixed in the KDR. |
| [`KDR-0014`](kdr/0014-interpolation-brace-escaping.md) | **accepted** — literal braces via doubling `{{` / `}}`, no backslash escape. |
| [`INDEX.md`](kdr/INDEX.md) | Both rows show `accepted`. |
| Issues | #5 (KDR-0013) and #6 (KDR-0014) closed with maintainer-acceptance comments. |

Explicitly **not** done (these are the follow-up PRs, in order, per root
[`AGENTS.md`](../AGENTS.md) hard rule 1 — spec → tests → impl, never mixed):

- No edit to [`keel-core.md`](spec/keel-core.md) yet (still proves only `+ * > < ==`).
- `K0204` not yet registered in [`keelc-diag`](../compiler/keelc-diag/src/registry.rs).
- No conformance cases for the new operators, division-by-zero panic, or `{{`/`}}`.
- No lexer/parser support for the new operators or brace doubling.
- Acceptance commit `129acd5` is **local only — not pushed**.

## Dependency Chain

1. Root [`AGENTS.md`](../AGENTS.md) — three-PR rule (spec → tests → impl), determinism, stable codes.
2. [`docs/spec/keel-core.md`](spec/keel-core.md) — the subset the new surface extends (§1 lexical, §2 types, §4 expressions).
3. [`docs/spec/00-spec-plan.md`](spec/00-spec-plan.md) — chapter numbering: `01-lexical` (KDR-0014), `02-types`/`04-expressions` (KDR-0013).
4. [`KDR-0013`](kdr/0013-core-operators-and-integer-division.md), [`KDR-0014`](kdr/0014-interpolation-brace-escaping.md) — the binding decisions.
5. Related accepted decisions the implementation must honor (stubs in [`INDEX.md`](kdr/INDEX.md)): KDR-0005 (panics uncatchable — div-by-zero panics) and KDR-0009 (no implicit conversions, no overloading — operands must share a type, `K0202`).
6. [`compiler/keelc-diag/src/registry.rs`](../compiler/keelc-diag/src/registry.rs) — append-only code registry where `K0204` lands.
7. [`tests/conformance/README.md`](../tests/conformance/README.md) + [`AGENTS.md`](../tests/conformance/AGENTS.md) — case format and numbering bands.
8. [`docs/m1-compiler-workspace.md`](m1-compiler-workspace.md) — frontend implementation state; its lexer/parser scope is what these KDRs grow (see its Next Work).

## Milestone Boundary

[`ROADMAP.md`](../ROADMAP.md) places the repo at M1 (frontend) heading into M2.
The two KDRs split across milestones:

- **Lexer (M1):** new operator tokens; `{{`/`}}` doubling in string literals.
- **Parser (M1):** operator precedence/associativity from KDR-0013; `!`/unary `-`.
- **Typecheck (M2):** operand type-equality (`K0202`), `?`/result typing already present.
- **Runtime (M3, Go backend):** truncating division, dividend-signed remainder,
  div/rem-by-zero panic (`K0204`), `checked_div`/`checked_rem`.

Do not implement runtime semantics (M3) while M1/M2 reject-routing is the active
milestone; encode them as conformance cases first.

## Validation Snapshot

```text
git status --short            # clean
git log --oneline -2
  129acd5 kdr: accept KDR-0013 ... and KDR-0014 ...   # local only
  cde20b8 tests: extend conformance test set          # origin/main
cargo run -p conformance-runner -- --check
  suite ok: 74 case(s), structure valid
```

KDR acceptance was docs-only; `scripts/preflight.sh` was not re-run because no
compiler code changed.

## Next Work

In strict order, each its own PR:

1. **Spec PR — KDR-0014:** open chapter `01-lexical` (or extend `keel-core.md` §1)
   stating `{{`→`{`, `}}`→`}`, lone `}` / unterminated `{` are lexical errors.
   Name the conformance cases it will add (band `0xx`).
2. **Spec PR — KDR-0013:** chapter `02-types`/`04-expressions` (or `keel-core.md`
   §2/§4): operator set, precedence table, truncation + dividend-signed `%`,
   div/rem-by-zero panic, `checked_div`/`checked_rem`. Register `K0204` at
   spec-writing time. Name the `1xx`/`4xx` cases.
3. **Conformance PRs:** add the cases the spec PRs named (accept: arithmetic,
   booleans short-circuit, `{{`/`}}`; reject: `K0202` on mixed operands; `K0204`
   is a runtime panic → M3-gated case via `case.toml`).
4. **Compiler PRs:** lexer tokens + brace doubling, then parser precedence, then
   M2 typing, then M3 lowering — each referencing the cases it makes pass.

Still-open spec questions surfaced during analysis (decide via KDR/issue before
writing cases — do not invent): reserved-word-as-identifier diagnostic code,
`match`-arm type-mismatch code (K0401 text is if/else-scoped), top-level
declaration order independence, `Float` NaN/Inf equality, recursive struct/enum
types, literal/nested patterns in `match`, unknown/duplicate struct field codes.
