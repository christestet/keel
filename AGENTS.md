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

- Rust: default rustfmt + clippy::pedantic (no per-crate clippy.toml exists
  yet; suppress locally or open a KDR for project-wide rules). No `unsafe` in
  keelc without a KDR.
- Commit messages: [Conventional Commits](https://www.conventionalcommits.org/),
  e.g. `feat(m2): typecheck exhaustive match (K0301, K0302)`. Type is
  `feat|fix|docs|refactor|test|chore|perf`; scope is the milestone or area
  (`m8`, `harness`, `pages`); error codes go in the summary when relevant.
  `!` after the type/scope (`feat(m9)!: ...`) marks a breaking change —
  pre-1.0, that means a language-surface removal, not an addition.
  release-please derives version bumps and `CHANGELOG.md` from these, so the
  type must match the actual change, not just read well.

## What "done" means

A task is done when: [`scripts/preflight.sh`](scripts/preflight.sh) is green (it
runs exactly what CI runs), new behavior has new conformance cases, `keel fmt`/pretty-printer
round-trips any syntax you added, and the PR description states which spec
section it implements.

## The agent harness (how this guidance scales)

This guidance is a layered harness, versioned and CI-checked like any other
code ([`scripts/check-harness.sh`](scripts/check-harness.sh)):

- **This file holds the global rules.** Directory-local rules live in nested
  `AGENTS.md` files (`compiler/`, `tests/conformance/`, `docs/spec/`,
  `docs/kdr/`, `examples/`), each with a `CLAUDE.md` symlink so Claude Code
  loads it automatically; Codex and other agents read `AGENTS.md` natively.
  Nested files only *add* local rules — on any apparent conflict, this file
  wins and the nested file has a bug.
- **`scripts/preflight.sh`** is the executable definition of done. Run it from
  the repo root before declaring any task complete.
- **`scripts/check-docs.sh`** rejects broken local files and section anchors,
  self-links, and public Markdown files that are unreachable from this
  repository's [`README.md`](README.md).
- **`.agents/`** holds the shared agent layer: a permission allowlist and slash
  commands (`/preflight`, `/new-case`, `/new-kdr`, `/wiki-note`,
  `/harness-audit`). `.claude` is a symlink to `.agents` so Claude Code and
  other agent surfaces load the same files instead of drifting.

### LLM wiki notes

Implementation status and "what happened / what depends on it / what comes
next" notes belong in standalone wiki-style files under `docs/`, not in
`README.md`, KDRs, specs, or conformance cases. These notes are non-normative
and should link the governing docs instead of restating them. A good note has:

- status: what is done and what is explicitly not done
- dependency chain: linked specs, KDRs, architecture docs, tests, and harness
  files that constrain the work
- milestone boundary: what the roadmap allows next
- validation snapshot: exact commands and summary lines
- next work: concrete entry points, without inventing behavior

### Growing the harness

The harness must grow with the repo. When a new top-level area or compiler
crate group needs its own rules: add an `AGENTS.md` there, symlink
`CLAUDE.md -> AGENTS.md` beside it, and register the directory in
`scripts/check-harness.sh` — CI fails if the pieces drift apart. Keep nested
files short (~30 lines): rules agents actually violate, not documentation;
prose belongs in the README/ARCHITECTURE file the nested `AGENTS.md` points
to. Shared agent commands live in `.agents/`; keep `.claude` as a symlink, not
a second copy. Harness changes are their own concern under hard rule 1 — never
bundle them with spec, conformance, or compiler changes.

### The improvement loop

The harness is code: it regresses unless failures feed back into it. Two
mechanisms keep it and the codebase current:

- **Event-driven.** Whenever agent work gets corrected — in review, by CI, or
  by the user — because guidance was missing, ambiguous, or too weak to prevent
  the mistake, the correction is not finished until the lesson lands in a
  follow-up harness PR. Encode it at the strongest layer that fits: a
  `scripts/check-*.sh` wired into `scripts/preflight.sh` and CI (deterministic,
  covers every agent) > `.agents/settings.json` permissions/hooks (Claude Code
  surface) > a rule line in the nearest `AGENTS.md` (prose, weakest).
- **Scheduled.** CI opens a monthly issue labeled `harness`
  ([`.github/workflows/harness-audit.yml`](.github/workflows/harness-audit.yml));
  whoever picks it up runs `/harness-audit`, which sweeps for prose rules that
  keep being violated, stale guidance, unregistered areas, and tooling
  friction. Close the issue with a one-line result even when nothing was found.
