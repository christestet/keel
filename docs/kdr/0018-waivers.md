# KDR-0018: Waivers — configurable in public only

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Linter and compiler rules may be waived only via a structured, visible,
expiring annotation:

```keel
// keel:waiver(rule: complexity, reason: "generated state machine", issue: "PLAT-2241", expires: edition-2031)
```

Every waiver requires: (1) a rule name, (2) a reason string, (3) a tracking
issue, (4) an expiry (date or edition). Expired waivers are compile errors.
All waivers appear in `keel audit`, test summaries, and build output.
Non-annotated suppression mechanisms (per-file config, CLI flags, comments
without the structured format) are not accepted.

## Context

A non-configurable linter is essential to Keel's "every repo looks the same"
promise, but real projects encounter genuine exceptions — generated code,
progressive migration, third-party integration idioms. Without an escape
valve, teams either fork the toolchain or disable rules globally. With a
trivial escape valve (silent comment-based suppression), waivers become
invisible debt.

The structured annotation makes waivers visible, accountable, and temporary.
The count appears in every build (`waivers: 3` next to `coverage: 94%`),
preventing accumulation. The corpus-level statistics on waiver usage are the
evidence stream that tells the language team which rules are wrong in practice
— closing the feedback loop described in vision.md §1.

## Alternatives considered

- **Global config file** (allowlist of suppressed rules per module). Rejected:
  invisible to code review, easy to forget, no per-occurrence accountability.
- **Non-expiring waivers** (reason + issue, no expiry). Rejected: permanent
  waivers become permanent lint debt. Codebases evolve; what was a legitimate
  exception in edition Y may be fixable in edition Y+3.
- **No waiver mechanism** (pure "linter is always right"). Rejected: makes the
  linter an adoption blocker. Generated code and migration phases need a path.

## Consequences

- Waivers are always visible, always flagged in audit, and always temporary.
- The waiver mechanism enables a "lint-budget" pattern: teams can enforce
  `waivers < 5` in CI as a quality gate.
- Generated code that triggers lints requires the codegen to emit waivers
  automatically — feasible since `keel gen` is part of the toolchain.

## Reopening clause

Corpus evidence that the structured annotation format imposes significant
friction on legitimate waiver use cases, or that the expiry mechanism causes
harmful code churn (rewriting code to avoid a waiver rather than to improve
it).
