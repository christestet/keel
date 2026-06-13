# KDR-0010: One formatter, zero options

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** toolchain

## Decision

Keel ships exactly one formatter (`keel fmt`) with zero configuration options.
Formatting is enforced at compile time. The formatter is the AST pretty-printer
— there is no second formatting code path.

## Context

Derived from [`docs/vision.md`](../vision.md) §1, Appendix A. Formatting
debates are the single largest source of wasted team time in languages with
configurable formatters. One canonical style means code looks the same across
every project, every company, every decade.

Making the formatter the AST pretty-printer (compiler/ARCHITECTURE.md rule 3)
ensures that the formatter and the compiler never diverge — they use the same
canonical output. This is Go's `gofmt` insight, extended: `gofmt` is a
separate tool; Keel's formatter *is* the pretty-printer.

## Alternatives considered

- **Configurable formatter** (Prettier, rustfmt, clang-format model). Rejected:
  formatting configuration is a source of team bikeshedding, ecosystem
  fragmentation (every project has a different `.prettierrc`), and toolchain
  complexity (option parsing, compatibility across versions).

- **No formatter** (style guide only). Rejected: style guides drift, code
  reviews become formatting debates, and the "five-year team" inherits whatever
  stylistic accident the original author configured in their editor.

- **Multiple formatters** (community competition). Rejected: ecosystem
  fragmentation, the "which formatter does this project use" onboarding tax,
  and the impossibility of cross-project code comparison.

## Consequences

- Every Keel codebase looks the same. Code review across projects, teams, and
  companies requires zero formatting adjustment.
- Formatting is not bikesheddable. There is no "our style" — the compiler
  enforces the one canonical style.
- `keel fmt` is the AST pretty-printer. There is no second formatter to keep
  in sync. Any tool that consumes the AST produces correctly formatted output
  for free.
- Some formatting choices will frustrate individual preferences (line width,
  brace placement). This is the mechanism working: collective consistency
  beats individual taste.

## Reopening clause

None; foundational.
