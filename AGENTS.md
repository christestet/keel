# AGENTS.md — Rules for LLM / agent contributors

You are implementing a programming language. The failure mode for agents on this
repo is plausible-looking code that silently diverges from the spec. These rules
exist to prevent that. They are not suggestions.

## Required reading order (before any change)

1. `docs/vision.md` — the design and its rationale
2. `docs/spec/keel-core.md` — the frozen subset you are implementing
3. `docs/kdr/` — decisions already made; do not relitigate them in code
4. `compiler/ARCHITECTURE.md` — pipeline, crate layout, iron rules
5. The conformance tests relevant to your task

## The prime directive

**The conformance suite is the definition of correct.** Not your training data
about how Go/Rust/Swift do it, not what seems reasonable. If the spec and a test
disagree, stop and open an issue — never "fix" one to match your assumption.

## Hard rules

1. **One concern per PR.** Spec changes, conformance-test changes, and compiler
   changes never mix in a single PR. A behavior change is three PRs in order:
   spec → tests → implementation.
2. **Every compiler PR must reference the conformance cases it makes pass**, and
   may not break any passing case. Run `cargo run -p conformance-runner` before
   declaring done. Paste the summary in the PR description.
3. **Never invent language features.** If the program you're testing needs a
   feature that isn't in `keel-core.md`, the test is wrong or premature — do not
   extend the parser "while you're there."
4. **Diagnostics: stable codes.** New errors register a `K####` code in
   `keelc-diag`'s registry file. Never reuse or renumber. Reject-tests match
   codes, not text; you may improve message text freely.
5. **No new dependencies** without a PR that justifies them. We build a language
   that preaches dependency discipline; the compiler practices it.
6. **No panics on user input.** Fuzzy/malformed source must produce diagnostics.
   `unwrap()` on anything derived from source text is a review-blocking defect.
7. **Determinism.** Same input → byte-identical output (diagnostics order,
   generated Go, formatter output). Sort, don't iterate hash maps into output.
8. **When uncertain, write the failing test first** and ask in the issue. A
   failing conformance test is a perfect, unambiguous question.

## Scope discipline

Work from `ROADMAP.md`. If you are on M2, you do not touch backend code. If a
task seems to require violating milestone order, the task is mis-scoped — say so.

## Style

- Rust: default rustfmt + clippy::pedantic minus documented exceptions in
  `compiler/lints.toml`. No `unsafe` in keelc without a KDR.
- Commit messages: `m2: typecheck exhaustive match (K0301, K0302)` — milestone
  prefix, imperative, error codes when relevant.

## What "done" means

A task is done when: `scripts/preflight.sh` is green (it runs exactly what CI
runs), new behavior has new conformance cases, `keel fmt`/pretty-printer
round-trips any syntax you added, and the PR description states which spec
section it implements.

## The agent harness (how this guidance scales)

This guidance is a layered harness, versioned and CI-checked like any other
code (`scripts/check-harness.sh`):

- **This file holds the global rules.** Directory-local rules live in nested
  `AGENTS.md` files (`compiler/`, `tests/conformance/`, `docs/spec/`,
  `docs/kdr/`, `examples/`), each with a `CLAUDE.md` symlink so Claude Code
  loads it automatically; Codex and other agents read `AGENTS.md` natively.
  Nested files only *add* local rules — on any apparent conflict, this file
  wins and the nested file has a bug.
- **`scripts/preflight.sh`** is the executable definition of done. Run it from
  the repo root before declaring any task complete.
- **`.claude/`** holds the Claude Code layer: a permission allowlist and slash
  commands (`/preflight`, `/new-case`, `/new-kdr`). Everything they invoke is a
  plain script or documented workflow, so other agents and humans share the
  same entry points.

### Growing the harness

The harness must grow with the repo. When a new top-level area or compiler
crate group needs its own rules: add an `AGENTS.md` there, symlink
`CLAUDE.md -> AGENTS.md` beside it, and register the directory in
`scripts/check-harness.sh` — CI fails if the pieces drift apart. Keep nested
files short (~30 lines): rules agents actually violate, not documentation;
prose belongs in the README/ARCHITECTURE file the nested `AGENTS.md` points
to. Harness changes are their own concern under hard rule 1 — never bundle
them with spec, conformance, or compiler changes.
