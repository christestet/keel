# 14 — Editions

This chapter is **normative**. It defines how Keel evolves: **exclusive editions
on a fixed cadence with mandatory mechanical migration**, decided in
[`KDR-0001`](../kdr/0001-editions.md). The manifest key that selects an edition
is [`06-modules-packages.md`](06-modules-packages.md) §6.2; this chapter fixes
the edition *machinery*. It does not restate the frozen rules in
[`keel-core.md`](keel-core.md); on any conflict, file an issue rather than
reconciling silently (the prime directive, root [`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified, not yet implemented.** Edition 1 is the only
edition; the machinery must exist in the compiler before 1.0 even though
edition 2 is years away ([`KDR-0001`](../kdr/0001-editions.md)). This chapter
governs the M7 edition work.

## 14.1 Editions and exclusivity

An **edition** names a coherent set of language idioms. Editions arrive on a
fixed **three-year cadence**, and each is **LTS** with a defined overlap window
([`KDR-0001`](../kdr/0001-editions.md)).

Editions are **exclusive**: when an edition replaces an idiom, the old idiom is a
**compile error** in the new edition — there is no permanent coexistence
(`K1403`). This is the mechanical form of the "one way to do things" promise: it
is scoped *per edition* and enforced, so an idiom cannot silently become three
over a decade. Edition 1 removes nothing (it is the first), so `K1403` is
registered but not yet triggered.

## 14.2 Declaring an edition

A package declares its edition with the `[package].edition` key in `keel.toml`
([`06-modules-packages.md`](06-modules-packages.md) §6.2). When the key is
omitted, the package is built against the toolchain's **current** edition. A
declared edition the toolchain does not recognize is **`K1401`**.

Every module in a package is on that package's edition. A codebase **cannot mix
eras within one module** — there is no per-module or per-file edition override,
so mixing is structurally impossible inside a package. Across the dependency
graph, packages on different editions interoperate freely: **old editions
compile forever** (the compiler supports all editions), so a dependency need
never migrate for a dependent to upgrade.

## 14.3 Backward compatibility and migration

The compiler retains support for every shipped edition indefinitely. Code on an
old edition keeps compiling forever; nobody is forced to migrate.

No edition ships unless `keel fix` can migrate the **entire public corpus
automatically with zero semantic diffs**. If a change cannot be migrated
mechanically, the change is redesigned until it can
([`KDR-0001`](../kdr/0001-editions.md)). The migration burden therefore falls on
the language team once, not on every user repeatedly. (The `keel fix` command
surface itself is specified with the toolchain; this chapter fixes only the
guarantee it must meet.)

## 14.4 Preview features

Experimental features fed by the RFC process ship only behind
`keel build --preview=<feature>`. A preview-gated feature used without that flag
is **`K1402`**. A build that enables any preview feature is **non-deployable**:
the toolchain refuses to run a preview-enabled binary outside a CI-marked build
([`KDR-0001`](../kdr/0001-editions.md), vision §5) — you may experiment, you may
not deploy a preview.

## 14.5 Examples (illustrative)

```toml
# keel.toml — explicit edition
[package]
name = "users_service"
version = "0.1.0"
edition = "1"
```

```toml
# keel.toml — edition omitted → built against the toolchain's current edition
[package]
name = "validate"
version = "0.1.0"
```

## 14.6 Error conditions

Registered (append-only) by the implementation PR in the `K14xx` band:

- **`K1401` — unknown edition.** `[package].edition` names an edition the
  toolchain does not recognize.
- **`K1402` — preview feature used outside a preview build.** A feature gated by
  `--preview=<feature>` is used without that flag enabled.
- **`K1403` — idiom removed in the active edition.** An idiom that a later
  edition replaced is used under that edition. Registered now; not triggered
  until an edition past 1 removes an idiom.

## 14.7 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `840-edition-declared` | accept | a manifest with `edition = "1"` builds |
| `841-edition-default` | accept | a manifest omitting `edition` builds against the current edition |
| `842-unknown-edition` | reject `K1401` | `edition = "99"` (unrecognized) |
| `843-preview-without-flag` | reject `K1402` | a preview-gated feature used without `--preview` |

## 14.8 Dependencies

- Decision: [`KDR-0001`](../kdr/0001-editions.md) (exclusive editions, mandatory
  mechanical migration — foundational, no reopening of the mechanism).
- Manifest surface: [`06-modules-packages.md`](06-modules-packages.md) §6.2
  (the `edition` key; this chapter owns its value validation and semantics).
- Frozen base: [`keel-core.md`](keel-core.md) (the Core idiom set is edition 1's
  starting point).
- Code registry: `K1401`–`K1403` are registered (append-only) in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  by the implementation PR.
