# 11 — Capabilities

This chapter is **normative**. It defines Keel's **capability system**: the six
capabilities every package declares, how the compiler enforces them statically
and transitively, and what `keel audit` reports. The decision and threat model
are [`KDR-0011`](../kdr/0011-package-capabilities.md); the package boundary and
the `capabilities` manifest key on which this chapter builds are
[`06-modules-packages.md`](06-modules-packages.md). On any conflict with
[`keel-core.md`](keel-core.md), file an issue rather than reconciling silently
(the prime directive, root [`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified, not yet implemented.** Enforcement and
`keel audit` land in the M7 implementation PR; until then, `use std.http` and
friends remain ungated as in M6.

The capability system is **purely static** — there is no runtime token, handle,
or permission check ([`KDR-0017`](../kdr/0017-function-capabilities.md) rejects
linear/token-passing schemes). `keel audit` is therefore exhaustive **without
running the program**, and the feature adds **zero runtime surface** that could
leak or fail at runtime.

## 11.1 The six capabilities

A capability names a category of system authority a package may exercise
([`KDR-0011`](../kdr/0011-package-capabilities.md)):

| Capability | Authority it grants |
|---|---|
| `net` | open network connections (sockets, HTTP, network databases) |
| `fs` | read or write the filesystem |
| `exec` | spawn external processes |
| `env` | read process environment variables |
| `ffi` | cross an `extern` boundary into non-Keel code |
| `unsafe-memory` | use memory operations outside Keel's safety guarantees |

These six are the closed set. A `capabilities` entry that is not one of them is
`K1111`. `ffi` and `unsafe-memory` are the **memory-safety boundary**: code that
can violate Keel's safety guarantees is reachable only through these two
capabilities, so `keel audit` names exactly the packages that could do so
([`KDR-0011`](../kdr/0011-package-capabilities.md); FFI detail is
[chapter 12 in the specification plan](00-spec-plan.md#chapter-plan)).

## 11.2 Standard-library capability map

Each compiler-known `std` module requires a fixed capability set. Importing the
module (`use std.x`) or calling its API obligates the package to declare every
capability in that set.

| `std` module | Required capabilities |
|---|---|
| `std.time` | *(none)* |
| `std.json` | *(none)* |
| `std.log` | *(none)* — writes to stdout only ([`15-stdlib-core.md`](15-stdlib-core.md) §15.25) |
| `std.config` | `env` — environment-variable loader (§15.31) |
| `std.http` | `net` |
| `std.sql` | `net`, `fs` |

`std.sql` requires **both** `net` and `fs`: the driver is selected at runtime by
the connection string ([`15-stdlib-core.md`](15-stdlib-core.md) §15.28.1), so the
compiler cannot statically know whether a given pool opens a file (SQLite,
[`KDR-0042`](../kdr/0042-sqlite-driver-modernc.md)) or a socket (Postgres). The
conservative, sound requirement is both. This table is normative; new `std`
modules extend it in their own spec chapters.

## 11.3 Package-level enforcement

A package's **declared** capabilities are its manifest's `capabilities` set
(§06.2). The rule:

> A package may exercise a capability only if it declares that capability.

Enforcement is static, at compile time, in two parts:

1. **Direct use.** Importing or calling a `std` module obligates its §11.2
   capabilities. A capability required by a reachable `std` use but absent from
   the manifest is **`K1110`**. (When function-level annotations exist —
   [`KDR-0017`](../kdr/0017-function-capabilities.md), deferred, §11.6 — this
   tightens to per-call-path reachability; for now it is per-package.)

2. **Transitive use.** If package *A* depends on package *B*, then *A* can reach
   everything *B* can. *A*'s declared capabilities must therefore include every
   capability *B* declares: `declared(A) ⊇ declared(B)` for each direct
   dependency *B*. A dependency that requires a capability the dependent has not
   declared is **`K1112`**. This is exactly KDR-0011's "a package without `net`
   cannot reach the socket API" — transitively, by construction.

A package that declares a capability it never exercises (directly or
transitively) is **over-declared**: a `keel audit` warning (§11.5), not an
error — over-declaration is a smell, not unsafe.

## 11.4 The effective capability set

The **effective** capability set of a package is the union of its own declared
capabilities and the effective sets of all its dependencies:

```text
effective(P) = declared(P) ∪ ⋃ { effective(D) : D ∈ dependencies(P) }
```

Because the dependency graph is acyclic (§06.4), this terminates. By the §11.3
transitive rule a well-formed build has `effective(P) = declared(P)` for every
package — the rollup never *exceeds* the declaration. The effective set is what
`keel audit` reports, and the recursion is evaluated in the deterministic
topological order of §06.4.

## 11.5 `keel audit`

`keel audit` prints, for the package being built, its effective capability set
and — for each capability — the dependency packages that contribute it. It runs
purely on manifests and the static call graph; it never executes the program.

Output is **deterministic** (root [`AGENTS.md`](../../AGENTS.md) hard rule 7):
capabilities listed in the fixed §11.1 order, contributing packages sorted by
name. One screen answers "which of my dependencies can open a network
connection?"

```text
$ keel audit
users_service 0.1.0
  net           self, http_client 1.2.0
  fs            self
  (env, exec, ffi, unsafe-memory: not present)

warnings:
  validate 0.1.0 declares `fs` but never exercises it (over-declared)
```

The exact column formatting is implementation-defined; the **content and
ordering** are normative. `--verbose` is reserved for the per-function report
that function-level annotations (§11.6) will enable.

## 11.6 Deferred: function-level annotations

[`KDR-0017`](../kdr/0017-function-capabilities.md) (function-level
`use capabilities(...)` that may only *restrict* the package declaration) is
**out of scope for this chapter**. The package declaration is the mandatory,
authoritative gate; function-level granularity layers on later without changing
anything above. No annotation syntax is specified here.

## 11.7 Error conditions

Registered (append-only) by the implementation PR in the `K11xx` band shared
with [`06-modules-packages.md`](06-modules-packages.md); this chapter uses
`K1110`–`K1112`. All arise from static analysis of manifests and source and are
diagnostics, never panics (hard rule 6).

- **`K1110` — undeclared capability used.** A package reaches a `std` API
  (§11.2) requiring a capability its manifest does not declare.
- **`K1111` — unknown capability name.** A `capabilities` entry is not one of
  the six in §11.1.
- **`K1112` — dependency requires undeclared capability.** A direct dependency
  declares a capability the dependent package does not (§11.3 transitive rule).

Over-declaration (a declared capability never exercised) is a **warning**
surfaced by `keel audit`, not one of these errors.

## 11.8 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `820-capability-declared-http` | accept | `use std.http` with `capabilities = ["net"]` builds |
| `821-capability-none-needed` | accept | `use std.json` / `std.log` with empty `capabilities` builds |
| `822-capability-sql-both` | accept | `use std.sql` requires `["net", "fs"]`; both declared builds |
| `823-undeclared-net` | reject `K1110` | `use std.http` with `capabilities = []` |
| `824-unknown-capability` | reject `K1111` | `capabilities = ["network"]` (not one of the six) |
| `825-transitive-undeclared` | reject `K1112` | dependency declares `net`, dependent does not |
| `826-sql-missing-fs` | reject `K1110` | `use std.sql` with only `["net"]` declared |

## 11.9 Dependencies

- Decisions: [`KDR-0011`](../kdr/0011-package-capabilities.md) (the capability
  set and threat model), [`KDR-0017`](../kdr/0017-function-capabilities.md)
  (function-level annotations — deferred; no runtime tokens),
  [`KDR-0042`](../kdr/0042-sqlite-driver-modernc.md) (why `std.sql` implies
  `fs`).
- Paired chapter: [`06-modules-packages.md`](06-modules-packages.md) (manifest,
  `capabilities` key, dependency graph; shared `K11xx` band).
- Stdlib surface mapped here: [`15-stdlib-core.md`](15-stdlib-core.md)
  §15.25 (`log` → stdout), §15.28 (`sql`), §15.31 (`config` → env).
- Related: [`09-concurrency.md`](09-concurrency.md) §9.7 (`spawn` introduces no
  new authority — a task runs under its enclosing function's capability budget),
  [chapter 12 in the specification plan](00-spec-plan.md#chapter-plan) (the
  `ffi` / `extern` boundary).
- Code registry: `K1110`–`K1112` are registered (append-only) in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  by the implementation PR.
