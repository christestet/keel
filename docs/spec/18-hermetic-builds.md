# 18 — Hermetic, reproducible builds

This chapter is **normative**. It defines the build-side supply-chain contract:
`keel build` is **hermetic** and **reproducible**, decided in
[`KDR-0105`](../kdr/0105-hermetic-reproducible-builds.md). It does not restate the
frozen rules in [`keel-core.md`](keel-core.md); on any conflict, file an issue
rather than reconciling silently (the prime directive, root
[`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified.** This chapter governs the M7 hermetic-build
work. The dependency-side half of the supply-chain story is capability
enforcement ([`11-capabilities.md`](11-capabilities.md)); together they make
building untrusted code safe by definition ([`docs/vision.md`](../vision.md) §3).

## 18.1 The contract

A `keel build` of a given set of inputs with a given toolchain version:

1. **Runs no package code at build time.** There are no build scripts or
   build-time hooks ([`KDR-0007`](../kdr/0007-no-build-scripts.md)); building a
   package never executes that package's (or a dependency's) code. `keel gen`
   ([`17-codegen.md`](17-codegen.md)) is an explicit command, not a build step,
   so it does not breach this.
2. **Reaches no network.** Every input is the declared toolchain plus the
   manifest's resolved dependencies ([`06-modules-packages.md`](06-modules-packages.md));
   a build never fetches anything.
3. **Depends on no undeclared host state.** The output is a function of the
   toolchain version and the manifest-pinned inputs only — not of the build
   directory path, wall-clock time, environment, or VCS metadata.
4. **Is byte-identical on rebuild.** Two clean builds of the same inputs with the
   same toolchain version produce byte-identical output (root
   [`AGENTS.md`](../../AGENTS.md) hard rule 7). This is a **testable contract**,
   not a best-effort aspiration: a divergence is a defect of the same severity as
   a miscompilation.

Because a build can neither run dependency code nor reach the network, **building
untrusted code is safe by definition** ([`KDR-0105`](../kdr/0105-hermetic-reproducible-builds.md)).

## 18.2 Path and metadata independence

Property 3 forbids the most common reproducibility leak: embedding the absolute
build path or VCS revision into the output. The Go-emitting backend
([`KDR-0102`](../kdr/0102-go-backend-first.md)) therefore builds with path
trimming and VCS stamping disabled, so the build directory (a per-invocation
temporary) cannot influence the bytes of the output. Any future backend must
likewise keep its output a pure function of the §18.1 inputs.

## 18.3 What "clean build" means

A *clean* build starts from the source and manifest-pinned inputs with no reuse
of prior build artifacts. The reproducibility contract (§18.1 property 4) is
stated over clean builds so it cannot be satisfied trivially by a cache returning
its own earlier output. Content-addressed caching is a permitted *consequence* of
byte-identical output, not a substitute for it.

## 18.4 No `K####` code

Hermeticity and reproducibility are **build properties**, not source-level
diagnostics: there is no program a user writes that "is" a reproducibility error.
A divergence surfaces as a failing reproducibility check (§18.5), not a `K####`
diagnostic, so this chapter registers no code.

## 18.5 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `850-build-reproducible` | accept (`mode = "repro"`) | two clean `keel build`s of the same program produce byte-identical binaries, and the binary runs and prints the expected output |

The `repro` runner mode builds the program twice into distinct outputs, asserts
the two are byte-identical, then runs one and matches its stdout.

## 18.6 Dependencies

- Decision: [`KDR-0105`](../kdr/0105-hermetic-reproducible-builds.md) (hermetic,
  reproducible builds as a testable contract).
- No build scripts: [`KDR-0007`](../kdr/0007-no-build-scripts.md) (the absence
  this contract relies on).
- Capability half of the supply chain: [`11-capabilities.md`](11-capabilities.md).
- Backend that must preserve determinism:
  [`KDR-0102`](../kdr/0102-go-backend-first.md) (Go-emitting backend) and any
  future native backend.
- Determinism rule: root [`AGENTS.md`](../../AGENTS.md) hard rule 7
  (same input → byte-identical output).
