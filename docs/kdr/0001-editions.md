# KDR-0001: Evolution via exclusive editions with mandatory mechanical migration

- **Status:** accepted
- **Scope:** language, toolchain, governance

## Decision
Keel evolves through editions on a fixed 3-year cadence. (1) When an edition
replaces an idiom, the old idiom is a **compile error** in the new edition — no
permanent coexistence. (2) No edition ships unless `keel fix` migrates the
entire public corpus automatically with zero semantic diffs; if the migration
cannot be mechanical, the change is redesigned. Old editions compile forever.
Each edition is an LTS for its overlap window.

## Context
Go's error-handling decade (proposals abandoned 2025) shows what happens with no
evolution mechanism; Go's `interface{}`/`any` and Rust's soft deprecations show
what happens when old ways never die: "one way to do it" silently becomes three.

## Alternatives considered
Rust-style editions without exclusivity (rejected: idiom permanence erodes the
core promise). SemVer-major breaks (rejected: ecosystem bifurcation, Python 2→3).
Eternal backward compatibility (rejected: design mistakes become permanent, Go).

## Consequences
The language team bears migration cost once instead of users repeatedly. Edition
machinery must exist in the compiler before 1.0. Some changes are impossible
because they cannot be mechanically migrated — that is a feature.

## Reopening clause
None for the mechanism. Cadence (3 years) reopens with corpus evidence of
migration fatigue or harmful feature backlog.
