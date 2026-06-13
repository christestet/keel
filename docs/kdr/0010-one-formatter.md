# KDR-0010: One formatter, zero options

- **Status:** accepted
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

## Reopening clause

None; foundational.
