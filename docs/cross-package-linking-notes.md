# Cross-package linking — implementation notes

Non-normative orientation for continuing the cross-package linking work. The
binding documents are [`KDR-0044`](kdr/0044-cross-package-symbol-linking.md),
spec [§6.4](spec/06-modules-packages.md), and the root
[`AGENTS.md`](../AGENTS.md) hard rules; this note only says what is built, what
is not, and where to pick up. It restates none of their decisions.

## Status (2026-07-06)

Cross-package **function calls and type references** link and run. A root
package that declares a path dependency can call `module.fn(...)`, annotate with
`module.Type`, and the dependency's functions, structs, and enums are compiled
into the build. A dependency function that uses its own package's structs/enums
internally works end-to-end; the root can pass and receive those values (field
access, `match`) without the parser supporting direct `module.Type{...}`
construction.

Landed together (single working tree, on `main`, no PR — a deliberate override
of [`AGENTS.md`](../AGENTS.md) hard rule 1's spec→tests→impl PR separation, at
the maintainer's instruction for this session):

- **KDR:** [`0044-cross-package-symbol-linking.md`](kdr/0044-cross-package-symbol-linking.md) — accepted.
- **Spec:** [§6.4 "Cross-package symbol linking"](spec/06-modules-packages.md)
  (and the old "dependency graph" section renumbered to §6.4a).
- **Conformance:** `tests/conformance/818-cross-package-call/` — accept case,
  `math.add(2,3)` from a path dependency prints `5`.
- **Compiler:** `compiler/keelc-driver/src/link.rs` (the linker) and
  `manifest::root_dependencies` in `compiler/keelc-driver/src/manifest.rs`
  (dependency resolution reused from the manifest loader).

## How it works today

A source-level merge in the driver, run after `manifest::check_workspace`
passes and before the build-cache stamp / query pipeline (see
`compiler/keelc-driver/src/lib.rs`). For a root with path dependencies,
`link::link`:

1. resolves each `[dependencies]` alias to its directory and manifest name;
2. for each root `use <alias>.<module>`, parses the dependency module file;
3. renames that module's top-level functions to `pkgname__fn` and its
   structs/enums to `PkgnameType` (declarations and every internal reference —
   call sites, struct literals, and type positions via `walk_module`'s type
   visitor), keyed off the dependency's `[package].name`;
4. rewrites the root's `module.fn(...)` (`MethodCall`) into a free call
   `pkgname__fn(...)` and its `module.Type` annotations into `PkgnameType`;
5. pretty-prints one merged module and feeds it to the existing single-source
   pipeline.

Two mangling schemes because the two namespaces have different lints: functions
use `pkgname__fn` (snake_case tolerates `_`), types use `PkgnameType` (K0101
forbids `_` in a type name, so the package prefix is PascalCased and concatenated
— see `mangle_type`). It is a no-op for a single file or a workspace whose root
makes no cross-package reference, so every pre-existing path stays byte-identical.
Determinism comes from `BTreeMap`/`BTreeSet` ordering (hard rule 7).

## Not done yet (the ceiling)

Documented in the `link.rs` module comment, [`feature-status.md`](feature-status.md),
and [`packages-and-capabilities.md`](packages-and-capabilities.md):

- **Enum variant-name collisions.** A dependency's `struct`/`enum` *type* names
  are mangled (`mangle_type` PascalCases the package prefix so the result stays a
  valid UpperCamelCase identifier — a `pkg__Type` form would trip `K0101`), but
  enum *variant* names are left bare (they live in a flat namespace and are
  referenced unqualified). Two packages that declare a variant of the same name
  collide when both are linked.
- **Root-side struct construction.** The root cannot write `dep.Point{...}`: that
  parses as a `Field` expression, not a struct literal, so construction stays
  inside the dependency. The root passes/receives dependency values instead.
- **Interpolated calls.** A call written inside a string interpolation
  (`"{dep.f()}"`) is not rewritten — the AST stores interpolation bodies as raw
  text, so the merge cannot see the call. Bind the result in a `let` and
  interpolate the local.
- **Diagnostics point at merged source.** An error inside a merged dependency
  maps to a line in the merged text, not the dependency file.

Upgrade path (also the [KDR-0044 reopening clause](kdr/0044-cross-package-symbol-linking.md)):
real module namespaces in the resolver and backend, so linking stops
round-tripping through the pretty-printer.

## Dependency chain

- Decision: [`KDR-0044`](kdr/0044-cross-package-symbol-linking.md), constrained by
  [`KDR-0011`](kdr/0011-package-capabilities.md) (package = capability boundary),
  [`KDR-0105`](kdr/0105-hermetic-reproducible-builds.md) and
  [`KDR-0107`](kdr/0107-oci-image-build.md) (why the merge stays one `package main`).
- Spec: [§6.3](spec/06-modules-packages.md) (module resolution) →
  [§6.4](spec/06-modules-packages.md) (this work).
- Pipeline: [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md).
  Capability enforcement (`manifest.rs`, `K1105`/`K1110`/`K1112`) is unchanged
  and still runs on the original graph before linking.

## Milestone boundary

Not a numbered [`ROADMAP.md`](../ROADMAP.md) milestone — it closes a gap under
the existing package slice (M6/M7). It unblocks genuinely multi-package
programs; the M6/M7 example services were multi-package only in manifest and
capability terms before this. It does not authorize registry dependencies,
lockfiles, or publishing (still out of scope, see
[`packages-and-capabilities.md`](packages-and-capabilities.md)).

## Validation snapshot

```
scripts/preflight.sh   → green
conformance            → 226 passed, 0 failed, 4 skipped (KEEL_MILESTONE=M9)
```

Beyond `818`: `819-cross-package-type` exercises a dependency struct + enum used
by dependency functions, returned to and matched in the root, with a
`module.Type` annotation. A dependency function calling a sibling dependency
function (`quad` → `double`) links correctly, and `812-path-dependency` (import
without a call) plus every single-file case stay byte-identical.

## Next work

Concrete entry points, in the repo's spec → tests → impl order per concern:

1. **Interpolated cross-package calls.** Requires the interpolation body to be a
   parsed expression rather than raw text (an AST/parse change) before the merge
   can reach it — scope it as its own concern.
2. **Housekeeping already flagged as stale:** the `examples/capability-audit`
   `main.keel` header comment and `docs/troubleshooting.md` still say linking
   does not happen.
3. **Governance:** the work shipped as one tree; if CI's concern-separation is
   wanted retroactively, split into the KDR / spec / tests / compiler commit
   sequence before opening any PR.
