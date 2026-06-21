# KDR-0105: Hermetic, reproducible builds

- **Status:** proposed
- **Date:** 2026-06-21
- **Scope:** toolchain

## Decision

`keel build` is **hermetic** and **reproducible**:

- **No arbitrary compile-time code execution.** There are no build scripts or
  build-time hooks ([`KDR-0007`](0007-no-build-scripts.md)); building a package
  never runs that package's code.
- **No network access during a build.** All inputs are the declared toolchain
  plus the manifest's resolved dependencies; a build never reaches the network.
- **No undeclared host dependence.** A build depends only on the toolchain
  version and manifest-pinned inputs, not on ambient host state.
- **Byte-identical output.** Two clean builds of the same inputs with the same
  toolchain version produce byte-identical output (root
  [`AGENTS.md`](../../AGENTS.md) hard rule 7). This is a testable contract, not a
  best-effort aspiration.

Because a build can neither run dependency code nor reach the network, **building
untrusted code is safe by definition** ([`docs/vision.md`](../vision.md) §3).

## Context

From [`docs/vision.md`](../vision.md) §3: "Builds themselves are sandboxed and
hermetic — no build scripts, no arbitrary code execution at compile time
(KDR-0007), so `keel build` on untrusted code is safe by definition." This KDR
records the **build-side** half of the supply-chain story; the dependency-side
half is capability enforcement ([`KDR-0011`](0011-package-capabilities.md)).
Together they close the loop: fetching a dependency cannot run code, building it
cannot reach the network or host, and what it may do at runtime is declared and
audited. [`KDR-0007`](0007-no-build-scripts.md) removed build scripts; this KDR
states the reproducibility *contract* that absence makes achievable.

## Alternatives considered

- **Sandboxed build scripts** (Bazel-style actions in a sandbox). Rejected:
  [`KDR-0007`](0007-no-build-scripts.md) already removed build scripts entirely;
  sandboxing a mechanism that does not exist is moot, and sandboxes leak.

- **Best-effort reproducibility** (reproducible "when possible"). Rejected:
  byte-identical rebuild is a CI-checkable contract; "best-effort" is not a
  contract and silently rots. Severity is the same as a miscompilation
  ([`docs/vision.md`](../vision.md) §7 sets that precedent for the compile-time
  budget).

- **Trusting the host toolchain / environment.** Rejected: dependence on ambient
  host state defeats cross-machine reproducibility, build caching, and provenance.

## Consequences

- CI can assert that two clean builds are byte-identical and treat a divergence
  as a release blocker.
- `keel build` on untrusted dependency code is safe without an external sandbox.
- Combined with capability enforcement and the SBOM
  ([`KDR-0011`](0011-package-capabilities.md)), the supply chain is auditable end
  to end — fetch, build, and run are each constrained and inspectable.
- Byte-identical output enables content-addressed build caching and verifiable
  build provenance.
- The Go-emitting backend ([`KDR-0102`](0102-go-backend-first.md)) and any future
  native backend must each preserve determinism for this contract to hold; the
  conformance and build-reproducibility checks guard it.

## Reopening clause

Evidence that strict byte-identical reproducibility is infeasible for a
legitimate, common backend workload without a scoped escape hatch — at which
point the escape hatch, its audit trail, and its CI marking are designed before
the guarantee is relaxed.
