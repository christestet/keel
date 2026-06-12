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
| [`KDR-0013`](kdr/0013-core-operators-and-integer-division.md) | **accepted + spec written** — boolean `&& \|\| !` (short-circuit), arithmetic `- / %`, truncate-toward-zero division, dividend-signed remainder, div/rem-by-zero **panics** under code `K0204`, total escape `checked_div`/`checked_rem -> Option<Int>`. Precedence/associativity fixed in the KDR and tabulated in [`spec/04-expressions.md`](spec/04-expressions.md). |
| [`KDR-0014`](kdr/0014-interpolation-brace-escaping.md) | **accepted + spec written** — literal braces via doubling `{{` / `}}`, no backslash escape. Encoded in [`spec/01-lexical.md`](spec/01-lexical.md); lexical code `K0004` registered. |
| [`INDEX.md`](kdr/INDEX.md) | Both rows show `accepted`. |
| Issues | #5 (KDR-0013) and #6 (KDR-0014) closed with maintainer-acceptance comments. |
| Spec chapters | [`01-lexical.md`](spec/01-lexical.md) (KDR-0014) and [`04-expressions.md`](spec/04-expressions.md) (KDR-0013) written; `K0004` and `K0204` registered in [`keelc-diag`](../compiler/keelc-diag/src/registry.rs). Each chapter names the conformance cases its test PR will add. |

Explicitly **not** done (these are the follow-up PRs, in order, per root
[`AGENTS.md`](../AGENTS.md) hard rule 1 — spec → tests → impl, never mixed):

- No conformance cases yet for the new operators, division-by-zero panic, or `{{`/`}}` (the spec chapters name them; the test PRs add them).
- No lexer/parser support for the new operators or brace doubling.

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
git log --oneline -3
  f4fb86c spec: expressions chapter — operators and integer division (K0204)
  9556126 spec: lexical chapter — interpolation brace escaping (K0004)
  7c71710 docs: add core-surface handoff note ...        # origin/main
./scripts/preflight.sh
  74 passed, 0 failed, 0 skipped
  preflight: green
```

The spec chapters added two stable codes (`K0004`, `K0204`) but no new
conformance cases yet, so the suite count is unchanged at 74. The two spec
commits are **local only — not pushed**.

## Next Work

In strict order, each its own PR:

1. **Conformance PR — KDR-0014 (band `0xx`):** add the cases named in
   [`spec/01-lexical.md` §1.3](spec/01-lexical.md): `014-brace-escape-literal`,
   `015-brace-escape-with-interpolation` (accept); `016-lone-close-brace`,
   `017-unterminated-interpolation` (reject `K0004`).
2. **Conformance PR — KDR-0013 (bands `1xx`/`4xx`):** add the cases named in
   [`spec/04-expressions.md` §4.5](spec/04-expressions.md): arithmetic/boolean
   accepts `112`–`119`, `120-mixed-operand-type` (reject `K0202`),
   `411-comparison-no-chaining` (reject `K0003`), and the M3-gated
   `121`/`122` div/rem-by-zero panics (`K0204`, via `case.toml`). The exact
   encoding of a *runtime* panic expectation is a conformance-format decision —
   confirm it against [`tests/conformance/README.md`](../tests/conformance/README.md)
   before writing, do not invent a new expectation file kind.
3. **Compiler PRs:** lexer tokens + brace doubling, then parser precedence, then
   M2 typing, then M3 lowering — each referencing the cases it makes pass.

Still-open spec questions surfaced during analysis (decide via KDR/issue before
writing cases — do not invent): reserved-word-as-identifier diagnostic code,
`match`-arm type-mismatch code (K0401 text is if/else-scoped), top-level
declaration order independence, `Float` NaN/Inf equality, recursive struct/enum
types, literal/nested patterns in `match`, unknown/duplicate struct field codes.
