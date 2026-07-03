# 06 — Modules and Packages

This chapter is **normative**. It defines the **package** — the unit of
distribution, dependency, and capability declaration — and how `use` paths
resolve across packages. The package boundary is the trust boundary that
[`KDR-0011`](../kdr/0011-package-capabilities.md) makes auditable; the manifest
is declarative data only, never executable, per
[`KDR-0007`](../kdr/0007-no-build-scripts.md). It does not restate the frozen
module rules in [`keel-core.md`](keel-core.md) §6; on any conflict with
`keel-core.md`, file an issue rather than reconciling silently (the prime
directive in the root [`AGENTS.md`](../../AGENTS.md)).

Capability **declaration syntax** is fixed here (the `capabilities` manifest
key); capability **semantics and enforcement** are
[`11-capabilities.md`](11-capabilities.md). This chapter and chapter 11 form one
feature and share the `K11xx` diagnostic band.

Implementation status: **specified, not yet implemented.** No `keel.toml` is
read by the current toolchain; `keelc run <file>` compiles a single file as an
implicit package (§6.1). This chapter governs the M7 package work; until it is
implemented, the single-file behavior of M0–M6 is unchanged.

## 6.1 Modules and packages

A **module** is one `.keel` file ([`keel-core.md`](keel-core.md) §6). A
**package** is a directory tree rooted at a `keel.toml` **manifest**: every
`.keel` file at or below that directory belongs to the package, up to (but not
into) any subdirectory that roots its own manifest.

A program compiled as a single file — `keelc run main.keel` with no manifest in
its directory — is an **implicit package**: name derived from the file stem,
**derived capability set** ([`11-capabilities.md`](11-capabilities.md) §11.4,
[`KDR-0043`](../kdr/0043-implicit-package-capability-trust-anchor.md)), no
dependencies. An implicit package can only be the compilation root — a
dependency path without a manifest is `K1106` (§6.4) — so it is the build's
**trust anchor**, not a supply-chain boundary. This preserves all M0–M6
behavior. The moment a directory contains a `keel.toml`, its files form an
**explicit package** governed by that manifest.

A package has exactly one manifest. A directory containing two manifest files,
or a `.keel` file that belongs to no package and no implicit single-file
invocation, is not a valid compilation input.

## 6.2 The manifest: `keel.toml`

The manifest is **TOML** ([`KDR-0007`](../kdr/0007-no-build-scripts.md): data,
not code — there are no build scripts, hooks, or expressions in a manifest).
Its schema is **closed**: every key is one of the following, and any other key
is an error (`K1104`), so a typo is caught, never silently honored.

```toml
[package]
name = "users_service"        # required; snake_case identifier (K0101 rules)
version = "0.1.0"             # required; "MAJOR.MINOR.PATCH"
edition = "1"                # optional; defaults to the current edition; see chapter 14
capabilities = ["net", "fs"]  # optional; defaults to []; see chapter 11

[dependencies]
shared = { path = "../shared" }   # path dependency (only form in this milestone)
```

- **`[package].name`** — required. A `snake_case` identifier
  (`[a-z_][a-z0-9_]*`). It is the first segment under which the package's own
  modules are addressed (§6.3) and the name a dependent uses to import it.
- **`[package].version`** — required. A three-part `MAJOR.MINOR.PATCH` string.
  Its only role in this milestone is identity in diagnostics and `keel audit`;
  version **resolution** (ranges, registries) is out of scope.
- **`[package].edition`** — optional, defaults to the toolchain's current
  edition. A string naming the language edition the package is written against
  ([`KDR-0001`](../kdr/0001-editions.md)). Its **value validation and semantics**
  are deferred to chapter 14 (editions); this chapter only admits the key into
  the closed schema. An unrecognized edition value is a chapter-14 diagnostic,
  not `K1104`.
- **`[package].capabilities`** — optional, defaults to `[]`. A set of capability
  names drawn from the six in [`11-capabilities.md`](11-capabilities.md).
  Order-insensitive and deduplicated. An entry that is not one of the six is
  `K1111` (chapter 11).
- **`[dependencies]`** — optional. Each entry maps a local **alias** to a
  dependency. The only form in this milestone is a **path dependency**,
  `alias = { path = "<relative-path>" }`, resolved relative to the manifest's
  directory. Registry dependencies are deliberately deferred (no resolver
  infrastructure exists yet; consistent with
  [`KDR-0020`](../kdr/0020-ecosystem-bootstrap.md)).

A manifest that is not valid TOML, or whose values have the wrong type, is
`K1102` — reported as a diagnostic, never a panic
([`AGENTS.md`](../../AGENTS.md) hard rule 6). A missing required field is
`K1103`.

## 6.3 Module resolution

A `use a.b.c` path resolves by its **first segment**:

- **`std`** — the compiler-known standard library
  ([`15-stdlib-core.md`](15-stdlib-core.md)). Importing a `std` module that
  carries a capability requirement is gated by chapter 11; resolution itself
  succeeds if the module exists.
- **the current package's own `name`** — addresses a module within this package.
  The mapping from file path to module path is the package-relative path with
  directory separators replaced by `.` and the `.keel` suffix dropped.
- **a `[dependencies]` alias** — addresses the root module of that dependency,
  and modules beneath it by the same path mapping.

A `use` whose first segment is none of these — i.e. names neither `std`, the
package itself, nor a declared dependency alias — is `K1105` (undeclared
dependency). Resolution is deterministic: the segment namespace is fixed before
resolution begins, so ordering of `use` statements or directory traversal never
affects the outcome ([`AGENTS.md`](../../AGENTS.md) hard rule 7).

## 6.4 The dependency graph

Path dependencies form a directed graph over packages. The graph is resolved by
reading each dependency's manifest at its `path`:

- A dependency `path` that does not contain a readable `keel.toml` is `K1106`.
- The graph must be **acyclic**. A cycle (a package reachable from itself
  through dependency edges) is `K1107`.
- Two distinct packages in the graph that declare the same `[package].name`
  collide: `K1108`. Package names are global identities within a build, so a
  diamond that resolves to one shared package is fine, but two different
  packages claiming one name is not.

Traversal order for any whole-graph operation (capability rollup §chapter 11,
`keel audit`) is a deterministic topological order, ties broken by package name
sort, so output is byte-identical across runs (hard rule 7).

## 6.5 Examples (illustrative)

A leaf package that touches the network and a local database, depending on a
pure helper package:

```toml
# users_service/keel.toml
[package]
name = "users_service"
version = "0.1.0"
capabilities = ["net", "fs"]

[dependencies]
validate = { path = "../validate" }
```

```toml
# validate/keel.toml
[package]
name = "validate"
version = "0.1.0"
# capabilities omitted → [] : a provably harmless, pure package
```

```keel
// users_service/main.keel
use std.http      // requires capability `net` — declared above
use std.sql       // requires `net` + `fs`   — declared above
use validate.email   // a module of the `validate` dependency
```

Omitting `"net"` from the manifest while importing `std.http` is a capability
violation (`K1110`, chapter 11). Importing `analytics.track` without an
`analytics` entry under `[dependencies]` is `K1105`.

## 6.6 Error conditions

The following are errors with stable `K####` codes, registered in the
accompanying implementation PR (`K11xx` is the next free band and is shared with
[`11-capabilities.md`](11-capabilities.md); this chapter uses `K1101`–`K1108`).
Every one of these arises from reading untrusted manifest or source text and is
therefore a **diagnostic, never a panic** ([`AGENTS.md`](../../AGENTS.md) hard
rule 6).

- **`K1101` — manifest required but absent.** A package operation (a build with
  dependencies or declared capabilities) finds no `keel.toml` where one is
  required.
- **`K1102` — malformed manifest.** `keel.toml` is not valid TOML, or a value
  has the wrong type for its key.
- **`K1103` — missing or invalid required field.** `[package].name` or
  `[package].version` is absent or malformed (bad identifier / bad semver
  string).
- **`K1104` — unknown manifest key.** A key outside the closed schema of §6.2
  appears in the manifest.
- **`K1105` — undeclared dependency.** A `use` path's first segment names
  neither `std`, the current package, nor a `[dependencies]` alias.
- **`K1106` — unresolved dependency path.** A `[dependencies]` entry's `path`
  has no readable `keel.toml`.
- **`K1107` — dependency cycle.** The dependency graph is not acyclic.
- **`K1108` — package name collision.** Two distinct packages in the graph
  declare the same `[package].name`.

## 6.7 Conformance cases this chapter introduces

Cases land with the test PR; the conformance runner gains a package-aware mode
(a case may carry a `keel.toml` and, for dependency cases, sibling package
directories).

| Case | Kind | Asserts |
|---|---|---|
| `810-package-manifest-minimal` | accept | a one-file package with a valid `keel.toml` builds |
| `811-implicit-single-file` | accept | a bare `.keel` file with no manifest still builds (implicit package) |
| `812-path-dependency` | accept | `use <alias>.<module>` resolves through a `[dependencies]` path entry |
| `813-malformed-manifest` | reject `K1102` | `keel.toml` with a TOML syntax error |
| `814-missing-name` | reject `K1103` | manifest lacking `[package].name` |
| `815-unknown-manifest-key` | reject `K1104` | manifest with a key outside the closed schema |
| `816-undeclared-dependency` | reject `K1105` | `use` of an alias absent from `[dependencies]` |
| `817-dependency-cycle` | reject `K1107` | two packages that depend on each other |

## 6.8 Dependencies

- Decisions: [`KDR-0011`](../kdr/0011-package-capabilities.md) (package is the
  capability/trust boundary), [`KDR-0007`](../kdr/0007-no-build-scripts.md)
  (manifest is declarative data, no build scripts),
  [`KDR-0020`](../kdr/0020-ecosystem-bootstrap.md) (path-first, registry later).
- Paired chapter: [`11-capabilities.md`](11-capabilities.md) — capability
  semantics, enforcement, and `keel audit`; shares the `K11xx` band.
- Frozen base: [`keel-core.md`](keel-core.md) §1 (identifier rules, `use`
  keyword), §6 (one module per file, `use std.x` imports).
- Code registry: `K1101`–`K1108` are registered (append-only) in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  by the implementation PR ([`docs/spec/AGENTS.md`](AGENTS.md)).
