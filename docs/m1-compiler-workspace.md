# M1 Compiler Frontend Workspace

Small wiki note for the current M1 compiler frontend: what exists, why it
exists, what it depends on, and what should happen next. This note is
non-normative; the governing language definition remains
[`docs/spec/keel-core.md`](spec/keel-core.md) and the executable spec remains
[`tests/conformance/`](../tests/conformance/).

## Status

Done in `chore(m1): scaffold compiler workspace crates`:

| Area | State |
|---|---|
| Workspace | [`Cargo.toml`](../Cargo.toml) includes the M1 frontend crates. |
| Lockfile | [`Cargo.lock`](../Cargo.lock) records the new zero-dependency packages. |
| Runner | [`compiler/conformance-runner`](../compiler/conformance-runner/) remains in the workspace. |
| Span crate | [`compiler/keelc-span`](../compiler/keelc-span/) exists as an empty library crate. |
| Diagnostics crate | [`compiler/keelc-diag`](../compiler/keelc-diag/) exists as an empty library crate. |
| Lexer crate | [`compiler/keelc-lex`](../compiler/keelc-lex/) exists as an empty library crate. |
| AST crate | [`compiler/keelc-ast`](../compiler/keelc-ast/) exists as an empty library crate. |
| Parser crate | [`compiler/keelc-parse`](../compiler/keelc-parse/) exists as an empty library crate. |

Added after the scaffold:

| Area | State |
|---|---|
| Spans | [`compiler/keelc-span`](../compiler/keelc-span/) defines source IDs, byte spans, spanned values, and line/column mapping. |
| Diagnostics | [`compiler/keelc-diag`](../compiler/keelc-diag/) defines diagnostic codes, severities, diagnostics, and an append-only registry for the M0/M1 codes currently referenced by the conformance suite. |
| Lexer | [`compiler/keelc-lex`](../compiler/keelc-lex/) tokenizes Keel Core source, skips line comments, preserves newlines, emits EOF, and reports lexical/syntax-frontier diagnostics such as `K0102` for semicolons and `K0906` for attributes. |
| AST | [`compiler/keelc-ast`](../compiler/keelc-ast/) defines declarations, types, blocks, statements, expressions, match arms, and patterns used by Core frontend parsing. |
| Parser | [`compiler/keelc-parse`](../compiler/keelc-parse/) parses modules, declarations, function signatures, types, blocks, expressions, match/catch arms, tests, and selected Core rejection forms with stable diagnostics. |
| Tests | Unit tests cover span mapping, lexer comment/semicolon behavior, parser function parsing, user-generic rejection, missing parameter type rejection, and `match` scrutinee/block disambiguation. |

Added in [`m1: add keelc check driver`](https://github.com/christestet/keel/pull/2):

| Area | State |
|---|---|
| Driver | [`compiler/keelc-driver`](../compiler/keelc-driver/) adds the `keelc` binary with `keelc check <main.keel>`, which reads one source file and runs lex+parse only. |
| Diagnostic output | `keelc check` emits stable `error[K####]` / `warning[K####]` diagnostics with `main.keel:N:C` spans. |
| Runner execution | [`compiler/conformance-runner`](../compiler/conformance-runner/) defaults to M1 syntax validation, invokes `keelc check`, and requires later semantic reject cases to parse successfully instead of faking M2 diagnostics in the parser. |

Not done yet: there is still no resolver, typechecker, KIR, Go backend, full
`keel` CLI, formatter implementation, or runtime execution of accept-cases.
After PR #2, the M1 exit criterion is an implementation candidate: every M0 case
is routed through lex/parse and either parses or emits the expected syntax-stage
`K####` code.

## Dependency Chain

Read order for compiler work:

1. [`AGENTS.md`](../AGENTS.md): global agent rules and definition of done.
2. [`docs/vision.md`](vision.md): language and tooling rationale.
3. [`docs/spec/keel-core.md`](spec/keel-core.md): frozen M0-M4 language subset.
4. [`docs/kdr/INDEX.md`](kdr/INDEX.md): accepted decisions and their status.
5. [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md): compiler pipeline,
   crate layout, and iron rules.
6. [`tests/conformance/README.md`](../tests/conformance/README.md): executable
   spec case format.
7. Relevant conformance cases in [`tests/conformance`](../tests/conformance/).

Compiler-specific local rules:

- [`compiler/AGENTS.md`](../compiler/AGENTS.md)
- [`tests/conformance/AGENTS.md`](../tests/conformance/AGENTS.md), when touching
  conformance cases

## Binding Decisions

The scaffold follows these accepted decisions:

| Decision | Why it matters here |
|---|---|
| [`KDR-0101`](kdr/0101-compiler-in-rust.md) | `keelc` is implemented in Rust; self-hosting is post-1.0. |
| [`KDR-0102`](kdr/0102-go-backend-first.md) | Backend work is Go-emission first, but it is not part of M1. |
| [`KDR-0002`](kdr/0002-no-async-await.md) | `async` / `await` are not language features; frontend diagnostics must preserve that. |
| [`KDR-0004`](kdr/0004-no-macros.md) | No macros, annotations, compile-time code execution, or reflection. |

## Milestone Boundary

[`ROADMAP.md`](../ROADMAP.md) puts the repo at M1 compiler frontend work.

M1 includes:

- source spans and file IDs
- diagnostic types and stable codes
- lexer
- AST
- recursive-descent parser with recovery

M1 excludes:

- resolver and typechecker
- KIR lowering
- Go backend
- `keel` CLI
- formatter implementation beyond AST pretty-printer foundations

M1 exit criterion: every M0 conformance case lexes and parses, or fails with the
right `K####` syntax code.

Do not start M2 resolver/typechecker work until the M1 driver PR is reviewed and
merged. After that, M2 is the next roadmap milestone.

## Validation Snapshot

Latest local validation for the M1 driver PR:

```text
scripts/preflight.sh
harness: ok
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p conformance-runner -- --check
suite ok: 61 case(s), structure valid
cargo build --release -p keelc-driver
cargo run -p conformance-runner -- --keelc target/release/keelc
conformance suite: full run
61 passed, 0 failed, 0 skipped
preflight: green
```

## Next Work

Immediate next work:

1. Review and merge the M1 driver PR.
2. Treat M1 as complete only after the PR lands with `scripts/preflight.sh`
   still green.
3. Start M2 in a separate compiler PR. The first scoped M2 item should be the
   resolver/typechecker skeleton that can route semantic conformance reject
   cases to the correct stage without touching backend code.
4. Keep semantic failures such as missing struct fields, assignment to immutable
   bindings, type mismatches, `?` context errors, and exhaustiveness in M2; do
   not backfill them into parser recovery.

Frontend surface grew after this note: KDR-0013 (operators `&& || ! - / %`,
precedence) and KDR-0014 (interpolation brace doubling `{{`/`}}`) were accepted
and now bind the lexer and parser. The implementation pipeline they unblock is
tracked in [`docs/core-surface-operators-and-interpolation.md`](core-surface-operators-and-interpolation.md);
land its spec and conformance PRs before adding the lexer/parser support.
