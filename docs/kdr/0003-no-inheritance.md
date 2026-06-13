# KDR-0003: No inheritance

- **Status:** accepted
- **Scope:** language

## Decision

Keel has no inheritance. Composition + interfaces (≤5 methods) are the only
polymorphism mechanism.

## Context

Derived from [`docs/vision.md`](../vision.md) §1. Inheritance creates fragile
base-class hierarchies, diamond problems, and implicit coupling that defeats
the five-year-readability goal. Composition is explicit and testable.

## Reopening clause

None; foundational.
