# 19 — OCI image build (`keel build --image`)

This chapter is **normative**. It extends the chapter-18 hermetic-build contract
to a deployable OCI artifact, decided in
[`KDR-0107`](../kdr/0107-oci-image-build.md). It does not restate the frozen
rules in [`keel-core.md`](keel-core.md); on any conflict, file an issue rather
than reconciling silently (the prime directive, root
[`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified.** This chapter governs the M9
`keel build --image` work. Everything in this chapter is additive to
[chapter 18](18-hermetic-builds.md): every §18.1 property still holds, scoped
from "the binary" to "the OCI artifact containing the binary."

## 19.1 The command

`keel build --image <entrypoint>` produces a single OCI artifact on local disk;
it does not push to a registry and does not require a Docker/Podman/BuildKit
daemon. There is no separate Dockerfile to author or maintain — the artifact is
a direct output of the toolchain, matching how `keel build` already produces a
binary without a linker script.

## 19.2 Forced target: static Linux binary

`--image` implies `GOOS=linux`, `CGO_ENABLED=0` for the Go-emitting backend
([`KDR-0102`](../kdr/0102-go-backend-first.md)), cross-compiling if the build
host differs, because OCI containers run on Linux container runtimes regardless
of build host. A dependency that cannot be statically linked for Linux under
these constraints fails the image build with **`K1901`** rather than silently
producing a dynamically linked or host-targeted artifact.

## 19.3 Image contents: one layer, no base

The produced image has exactly **one filesystem layer**: the built binary at a
fixed path, and only files the build graph declares the entrypoint needs (none,
today). There is no base-OS layer — this is the `FROM scratch` shape named in
[`docs/vision.md`](../vision.md) §9, produced by the toolchain instead of a
hand-written Dockerfile:

- No base image is pulled from a registry. A base layer is an external input by
  digest, which would reintroduce the network dependency chapter 18 removed.
- The image does not carry CA certificates, timezone data, a shell, or
  `/etc/passwd` entries. A service that needs any of these declares them as
  ordinary build inputs; the toolchain does not add them implicitly.
- The image config's `Entrypoint` is the built binary; `WorkingDir` is `/`;
  `User` is a fixed non-root numeric UID:GID. No health-probe wiring,
  SIGTERM-handling installation, or Helm/manifest generation happens here —
  those are explicitly out of scope for M9 (`ROADMAP.md`).

## 19.4 Format: OCI Image Layout

The artifact conforms to the [OCI Image Layout
spec](https://github.com/opencontainers/image-spec/blob/main/image-layout.md):
an `oci-layout` marker file, `index.json`, and content-addressed blobs under
`blobs/sha256/`. `keel build --image <entrypoint> -o <path>` writes this layout
to a directory at `<path>`; `-o <path>.tar` writes the single-file `oci-archive`
tar form of the same layout, importable by `docker load`, `podman load`, or
`skopeo copy` without Keel's toolchain being involved in that step.

Media types used are the plain OCI ones, uncompressed:

- Layer: `application/vnd.oci.image.layer.v1.tar` (no `+gzip` — compression adds
  a nondeterminism source, KDR-0107's decision, for no benefit to a
  single-binary layer already produced by a deterministic build).
- Config: `application/vnd.oci.image.config.v1+json`.
- Manifest: `application/vnd.oci.image.manifest.v1+json`.
- Index: `application/vnd.oci.image.index.v1+json`.

## 19.5 Determinism inputs

Extending §18.1 property 4 (byte-identical rebuild) to the image artifact means
every constructor of every layout file is a pure function of the §18.1 inputs
plus the forced target platform (§19.2):

- Tar entries (the one layer) use a fixed mtime (Unix epoch 0) and fixed
  numeric uid/gid (0:0), regardless of the build host's clock or the invoking
  user, and are written in a fixed, deterministic order (root
  [`AGENTS.md`](../../AGENTS.md) hard rule 7: sort, don't iterate hash maps into
  output).
- The image config JSON has no `created` timestamp field (the OCI spec permits
  omitting it) and no host-derived `author`/`os.version` field; JSON object keys
  are emitted in a fixed, sorted order.
- `index.json` and the manifest reference blobs by their sha256 digest only —
  no embedded build path, hostname, or VCS metadata, matching §18.2.

Two clean `keel build --image` invocations of the same inputs on the same
toolchain version, on any host, produce a byte-identical layout: identical
`oci-layout`, identical `index.json` bytes, identical manifest and config blobs,
identical layer blob, and therefore an identical top-level image digest. This is
a **testable contract**, exactly as chapter 18 states for the plain binary: a
divergence is a defect of the same severity as a miscompilation.

## 19.6 No daemon, no network, no registry

Building an image reaches no network and starts no daemon process — it is pure
local computation over the already-built binary, same as writing any other
build output file. Pushing the produced artifact to a registry is a distinct,
user-invoked step outside `keel build --image` (e.g. `skopeo copy` against the
written layout) and is not specified here. Because no step in the image path
executes package code or touches the network, building an untrusted workspace
into an image is safe by construction, the same guarantee chapter 18 states for
the binary ([`KDR-0105`](../kdr/0105-hermetic-reproducible-builds.md)).

## 19.7 Diagnostics

| Code | Meaning |
|---|---|
| `K1901` | `--image` target cannot produce a static Linux binary (a dependency requires cgo or another dynamic/host-specific link requirement) |

Reproducibility and layout-validity failures are **build properties**, like
chapter 18's byte-identity check, not `K####` diagnostics: there is no Keel
program a user writes that "is" a non-reproducible image. `K1901` is the one
genuinely diagnosable, source/manifest-attributable condition this chapter
introduces.

## 19.8 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `860-image-reproducible` | accept (`mode = "image"`) | two clean `keel build --image` runs of the same program produce a byte-identical OCI layout (identical top-level digest) |
| `861-image-layout-valid` | accept (`mode = "image"`) | the produced layout parses as a valid OCI Image Layout: `oci-layout` marker, `index.json`, manifest, and config conform to the OCI Image Spec and reference an existing single-layer blob |
| `862-image-no-base-layer` | accept (`mode = "image"`) | the produced manifest lists exactly one layer, and the config's rootfs diff-ids list has exactly one entry |
| `863-image-cgo-dependency` | reject (`K1901`) | a program whose dependency requires cgo fails `--image` with `K1901` instead of producing a dynamically linked artifact |

The `image` runner mode invokes `keelc build --image`, then validates the
written layout against §19.4 and, for `860`, builds twice into distinct output
paths and asserts the two layouts are byte-identical (mirroring the `repro`
mode chapter 18 introduced for the plain binary).

## 19.9 Dependencies

- Decision: [`KDR-0107`](../kdr/0107-oci-image-build.md) (daemonless,
  reproducible OCI image build).
- Extends: [`18-hermetic-builds.md`](18-hermetic-builds.md) and
  [`KDR-0105`](../kdr/0105-hermetic-reproducible-builds.md) (the binary-level
  hermetic/reproducible contract this chapter scopes up to an image).
- No build scripts: [`KDR-0007`](../kdr/0007-no-build-scripts.md).
- Backend that must produce the forced static Linux target:
  [`KDR-0102`](../kdr/0102-go-backend-first.md).
- Landing-kit context (Dockerfile/Helm mentioned but out of scope for this
  chapter): [`docs/vision.md`](../vision.md) §9, [`KDR-0020`](../kdr/0020-ecosystem-bootstrap.md).
- Determinism rule: root [`AGENTS.md`](../../AGENTS.md) hard rule 7.
