# KDR-0108: `keel build --image --arch` target-architecture selection

- **Status:** accepted
- **Date:** 2026-07-04
- **Scope:** toolchain

## Decision

`keel build --image` accepts an optional `--arch` flag selecting the target CPU
architecture of the produced OCI image, one of `amd64` or `arm64`, defaulting to
`amd64` when omitted. The selected architecture sets **both** the Go backend's
`GOARCH` for the forced static-Linux cross-compile (extending
[`KDR-0107`](0107-oci-image-build.md)'s `GOOS=linux`, `CGO_ENABLED=0`) **and**
the `architecture` field of the emitted OCI image config, so the two can never
disagree. Each invocation produces a **single-architecture** image; a multi-arch
image index carrying several platforms in one artifact is explicitly not decided
here. An unrecognized `--arch` value is a usage error (exit code 2), consistent
with how the driver already rejects a stray `-o` or a misused `--image`, not a
`K####` diagnostic — the set of valid values is a fixed CLI enumeration, not a
property of a Keel program.

## Context

KDR-0107 forced `GOOS=linux` for `--image` but explicitly deferred architecture
selection ("with a future `--target` for arch selection, not decided here",
§Alternatives). The implementation that shipped left `GOARCH` unset, so the Go
toolchain defaulted it to the **build host's** architecture while the config
writer hard-coded `"architecture":"amd64"`. On an arm64 build host (Apple
Silicon under OrbStack/Docker Desktop, AWS Graviton CI) this emitted a
linux/arm64 binary mislabeled as amd64: the runtime on that same host routes an
`amd64`-declared image through x86 emulation, which then fails to execute the
arm64 ELF, and the layer blob diverged between amd64 and arm64 build hosts,
breaking [ch19 §19.5](../spec/19-oci-images.md)'s "byte-identical on any host"
contract. Pinning `GOARCH=amd64` closes the correctness bug; this KDR decides the
forward-looking piece KDR-0107 left open — how a user asks for arm64 on purpose —
now that arm64 Linux is a first-class deployment target (Graviton, Ampere,
arm64 Kubernetes nodes), not an edge case.

## Alternatives considered

- **Multi-arch image index (both platforms in one artifact).** Rejected for M9:
  a manifest-list/index carrying linux/amd64 and linux/arm64 doubles the
  `go build` cost per image, requires the writer to emit two configs, two layers,
  and per-manifest `platform` descriptors, and pushes the reproducibility
  contract from "one layout" to "N layouts + an index" all at once. The single
  static binary makes per-arch builds cheap and scriptable (`--arch amd64` then
  `--arch arm64`), and CI multi-arch push is a registry concern KDR-0107 already
  placed out of scope. Not forbidden later — this decision leaves the index shape
  open for its own KDR when registry push is scheduled.
- **Detect and label the build host's architecture automatically.** Rejected: it
  reintroduces the exact host-dependence bug this decision exists to remove — the
  same source would produce an amd64 image on one machine and an arm64 image on
  another with no flag naming the difference, and §19.5 promises a layout that is
  a pure function of inputs *plus the explicitly selected target platform*, not of
  who ran the build.
- **A general `--target <os>/<arch>` string like `ko`/`docker buildx`.** Rejected
  for now: `GOOS` is already forced to `linux` by KDR-0107 (OCI images run on
  Linux runtimes), so the only free dimension is arch. A single-purpose `--arch`
  with a two-value enumeration is the smaller surface; a full `--target` grammar
  is unjustified until a second OS or a wider arch set is actually in scope.
- **A new `K####` diagnostic for an invalid `--arch` value.** Rejected: reject
  diagnostics describe defects in a *Keel program*. A bad CLI flag value is not a
  program property; the driver already handles analogous CLI misuse (`-o` without
  `--image`, `--image` on a non-`build` command) as plain usage errors with exit
  code 2, and `--arch` follows that established path.

## Consequences

- arm64 Linux becomes a supported first-class `keel build --image` target;
  users on arm64 hosts get a natively-runnable image, and users targeting
  Graviton/Ampere from an amd64 host get a correct cross-compiled one.
- The determinism contract strengthens: the emitted architecture is now an
  explicit, named input to the layout rather than an implicit host default, so
  §19.5's "byte-identical on any host" holds for each `--arch` value
  independently. A new conformance case asserts the arm64 path is reproducible;
  the existing amd64 case (`860-image-reproducible`) continues to cover the
  default.
- The conformance runner's image mode gains an optional per-case `arch` knob so a
  case can exercise a non-default target; this is test-harness surface, not
  language surface.
- Multi-arch image indexes, registry push, and any OS other than Linux remain out
  of scope and unauthorized by this KDR (as under KDR-0107); each needs its own
  slice when scheduled.

## Reopening clause

Evidence that a single-architecture-per-invocation model blocks a real workflow
that per-arch scripting cannot serve — e.g. a measured need to distribute one
artifact resolvable to multiple platforms *without* a registry (the registry
push path, when specified, would supply the index natively), or a supported
deployment target whose architecture is neither `amd64` nor `arm64` and cannot be
added as a further enumeration value. Demand for `docker buildx`-style
multi-platform ergonomics, on its own, is not evidence; a demonstrated workload
`--arch` cannot express is.
