# KDR-0003: No inheritance

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** language

## Decision

Keel has no inheritance. Composition + interfaces (≤5 methods) are the only
polymorphism mechanism.

## Context

Derived from [`docs/vision.md`](../vision.md) §1. Inheritance creates fragile
base-class hierarchies, diamond problems, and implicit coupling that defeats
the five-year-readability goal. Composition is explicit and testable.

Go proved that a mainstream backend language can thrive without inheritance.
Interfaces with a small method limit force deliberate, documented abstractions
instead of "I'll just override this one method" patterns that accumulate into
untraceable behaviour.

## Alternatives considered

- **Single inheritance with virtual methods** (Java/C# model). Rejected: fragile
  base-class problem — a change to a base class can silently alter subclass
  behaviour in production, unreachable statically. Violates the "illegal states
  unrepresentable" principle.

- **Mixins / traits** (Scala, Rust model). Rejected: diamond resolution rules
  add compiler complexity and a new concept. Diamond problems are a hard
  constraint on refactoring: extracting a common base from two trait users
  can change behaviour.

- **Structural subtyping** (OCaml, TypeScript model). Rejected: nominal
  subtyping is essential for the "five-year team" goal — you must be able to
  find every implementor of an interface by name. Structural subtyping makes
  implicit coupling a feature.

## Consequences

- All polymorphism is explicit via interface declarations and implementations.
  You can `grep` for every type that satisfies a given interface.
- Testing is simpler: mock objects are explicit structs implementing an
  interface, not inheritance chains.
- Some code that would use inheritance in other languages (default method
  implementations, template method pattern) must be written as composition.
  This is intentional — composition is testable, verifiable, and grepable.

## Reopening clause

None; foundational.
