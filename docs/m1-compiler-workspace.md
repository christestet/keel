# M1 Compiler Workspace

Small wiki note for the current M1 compiler scaffold: what exists, why it
exists, what it depends on, and what should happen next.

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

Not done yet: no lexer, parser, AST model, diagnostics registry, resolver,
typechecker, KIR, backend, or driver behavior has been added.

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

## Current Milestone Boundary

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

## Validation Snapshot

Latest local validation for this scaffold:

```text
scripts/preflight.sh
preflight: green

cargo run -p conformance-runner -- --check
suite ok: 61 case(s), structure valid
```

## Next Work

Start with [`compiler/keelc-span`](../compiler/keelc-span/) and
[`compiler/keelc-diag`](../compiler/keelc-diag/). The lexer and parser should
depend on those foundations instead of inventing local span or error shapes.
