# KDR-0107: Daemonless, reproducible OCI image build (`keel build --image`)

- **Status:** accepted
- **Date:** 2026-07-04
- **Scope:** toolchain

## Decision

`keel build --image` produces an [OCI Image Layout](https://github.com/opencontainers/image-spec/blob/main/image-layout.md)
artifact directly from the compiler process, with no Docker/Podman/BuildKit
daemon and no base-image pull:

- **One layer, the static binary, nothing else.** The image contains a single
  filesystem layer holding the already-built executable (and only files the
  build graph says the service needs — none yet). There is no base OS layer:
  this is the `FROM scratch` shape named in [`docs/vision.md`](../vision.md)
  §9, produced by the toolchain instead of a hand-written Dockerfile.
- **The backend target is forced to a static Linux binary.** `--image` implies
  `GOOS=linux`, `CGO_ENABLED=0`, cross-compiling if the build host differs.
  An image build with a dependency that cannot produce a static Linux binary
  (e.g. requires cgo) fails with a diagnostic rather than emitting a
  non-portable image.
- **Every byte of the artifact is a pure function of the §18.1 inputs plus the
  target platform.** No wall-clock timestamps, no build-host UID/GID/paths, no
  network fetch of a base image or registry metadata. Tar entries use a fixed
  mtime and fixed ownership; JSON (config, manifests, index) is emitted with
  sorted keys and no map-iteration nondeterminism (root
  [`AGENTS.md`](../../AGENTS.md) hard rule 7). This extends the
  [`KDR-0105`](0105-hermetic-reproducible-builds.md) contract from "binary" to
  "binary packaged as an OCI artifact."
- **No registry interaction.** `keel build --image` writes a local artifact
  (an OCI image layout directory, or the single-file `oci-archive` tar form of
  it); pushing to a registry is a separate, later concern and is not decided
  here.
- **The image config is minimal and inferred, not authored.** `Entrypoint` is
  the built binary; `Env` carries only variables the manifest's
  [`std.config`](../spec/15-stdlib-core.md) declarations require to be visible
  as documentation, not injected secrets. No health-probe wiring, no shutdown
  machinery, no user/group scaffolding beyond a fixed non-root numeric UID —
  those stay explicitly out of scope for M9 per `ROADMAP.md`.

## Context

M9 extends the chapter-18 hermetic-build contract
([`KDR-0105`](0105-hermetic-reproducible-builds.md)) to a deployable artifact.
`docs/deployment.md` records the current gap plainly: the driver has no
target-selection interface (`GOOS`/`GOARCH` are not exposed), does not force
`CGO_ENABLED=0`, and does not emit or test container images — "that manual
image is outside the current hermetic/reproducibility guarantee." Today,
users who want a container hand-write a Dockerfile and run `docker build`,
which reintroduces everything KDR-0007 and KDR-0105 removed: a base-image pull
from a registry (network access, a mutable dependency outside the manifest), a
build daemon as an undeclared host dependency, and non-reproducible layer
timestamps/ordering.

The industry's standard tools for daemonless image construction (Kaniko,
Buildah, `img`, ko, Bazel's `rules_oci`) all validate the same shape: a single
static binary plus a hand-rolled OCI writer needs no daemon and no shelling out
to `docker build`. `ko` in particular is the closest precedent — it builds Go
binaries straight into OCI images without a Dockerfile or daemon — but it still
depends on external registries for base images by default. Keel has no base
image to begin with, which removes that dependency entirely and makes
byte-identical rebuild strictly easier to guarantee than any of these tools
commit to.

## Alternatives considered

- **Shell out to `docker build` / BuildKit.** Rejected: requires a Docker
  daemon (an undeclared host dependency KDR-0105 already forbids for the plain
  binary build), typically pulls a base image over the network, and BuildKit's
  own layer/cache metadata is not specified to be byte-identical across hosts.
  Adopting it would make the image path strictly less reproducible than the
  binary path it wraps.
- **Multi-layer image with a minimal base OS (distroless-style).** Rejected:
  a base layer is an external input pulled by digest from a registry — network
  access at build time, and a mutable trust dependency the manifest does not
  declare. `keel build --image` builds when disconnected from the network by
  construction (root `AGENTS.md` hard rule 7's sibling guarantee, chapter 18
  property 2); the "FROM scratch" shape is what makes that possible.
  Application-level base needs (CA certs, timezone data) stay an explicit,
  user-authored addition on top of the toolchain artifact, not a toolchain
  default — `docs/deployment.md` already tells users not to assume those files
  exist in `FROM scratch`.
- **Depend on an existing Rust OCI-building crate (e.g. an `oci-spec`/registry
  client library).** Rejected for now: the OCI image-layout format needed here
  is a small, frozen JSON schema plus a tar writer, well within "a few lines"
  territory per root `AGENTS.md` hard rule 5 ("no new dependencies without a
  PR that justifies them"). A hand-rolled writer keeps the determinism
  contract fully under the conformance suite's control instead of a
  dependency's release cadence. This KDR does not forbid a future dependency
  PR if the hand-rolled writer proves to be the wrong call — that PR still has
  to justify itself on its own, most likely for the SHA-256 digest computation
  the OCI spec requires (a well-audited crate is preferable to hand-rolled
  cryptography), not for the tar/JSON structure itself.
- **Let `--image` target the build host's OS/arch like the existing binary
  build does.** Rejected: OCI images run on Linux container runtimes
  regardless of build host, so a macOS or Windows build host producing a
  host-targeted image would be silently wrong. Forcing `GOOS=linux` for
  `--image` (with a future `--target` for arch selection, not decided here)
  matches what every competing tool (`ko`, Kaniko) does by default.

## Consequences

- `docs/deployment.md`'s documented gap ("Keel does not yet emit or test
  container images") closes; its "minimal container shape" section becomes
  normative toolchain behavior instead of manual advice.
- A new image-conformance mode is required: build the same workspace twice and
  assert byte-identical OCI digests, validate OCI layout/config against the
  spec, and assert no network socket, no Docker daemon socket, and no
  timestamp/build-path/host metadata reaches the artifact — mirroring how
  conformance case 850 already checks binary byte-identity.
- Cross-compilation (forcing `GOOS=linux`, `CGO_ENABLED=0`) becomes a
  toolchain-owned concern rather than an application concern; a dependency
  that cannot be statically linked for Linux must fail `--image` builds with a
  diagnostic (new `K####` code, per root `AGENTS.md` hard rule 4) rather than
  silently producing a dynamically linked or non-Linux artifact.
- Registry push, Helm charts, health-probe wiring, and image signing remain
  explicitly out of scope (per `ROADMAP.md`'s M9 section) and are not
  authorized by this KDR; they need their own KDR/spec/implementation slices
  when scheduled.
- The image path inherits every consequence of
  [`KDR-0105`](0105-hermetic-reproducible-builds.md): CI can treat a
  byte-identical-digest failure as a release blocker, and building an
  untrusted workspace into an image stays safe by construction because no
  step in the new path executes package code or touches the network.

## Reopening clause

Evidence that the single-layer, no-base-image, hand-rolled-writer shape is
infeasible for a common, legitimate workload — e.g. a service that
demonstrably needs OS-level files (CA certificate bundles, timezone data,
`/etc/passwd` entries) that cannot be supplied as ordinary build inputs
declared through the existing manifest, or a measured case where hand-rolling
the OCI writer produces a correctness defect a maintained crate would have
prevented. "Other languages ship a Dockerfile" is not evidence; a demonstrated
gap in what `keel build --image` can express is.
