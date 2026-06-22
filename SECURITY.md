# Security policy

Keel is pre-1.0 research software. It has no supported release line or published
binary distribution. Security work currently targets the repository's active
development branch; old commits and milestone snapshots receive no security
backports.

Do not deploy the current toolchain or generated services as though they carry a
stable production security guarantee.

## Reporting a vulnerability

Prefer the repository host's private security-advisory channel when it is
available. Include:

- the affected commit;
- the smallest source/manifest/schema that reproduces the issue;
- the exact command and host platform;
- observed and expected behavior;
- impact and any known exploitation conditions.

Do not put exploit details, secrets, or sensitive production data in a public
issue. If no private advisory channel is available, open a minimal public issue
requesting private maintainer contact without describing the vulnerability.

Compiler crashes on untrusted source, capability bypasses, nondeterministic
security output, generated-code injection, and reproducibility leaks are
security-relevant even when they do not immediately yield code execution.

## Current security boundaries

### Source processing

The compiler is written in safe Rust and project policy forbids `unsafe` without
a KDR. Malformed Keel source, manifests, and schemas are expected to produce
diagnostics rather than panic. A panic caused by user-controlled input is a bug.

This policy is enforced by review and tests, not by a claim that the compiler
has undergone a security audit.

### Generated programs

The current backend emits Go and relies on the Go runtime for memory management,
scheduling, networking, and process behavior. Keel's language surface has no
implemented FFI or unsafe-memory operation at M7. The planned FFI will be an
explicit safety boundary, but it provides no protection today because it does
not exist.

### Package capabilities

Explicit packages declare `net`, `fs`, `exec`, `env`, `ffi`, and
`unsafe-memory`. The M7 driver scans local package source and dependencies,
rejects undeclared direct/transitive authority, and exposes contributors through
`keel audit`.

Current limitation: a source file without adjacent `keel.toml` is an implicit
package and bypasses capability enforcement. Its audit output reports no
capabilities even when it imports a capability-bearing standard module. Use an
explicit manifest before treating capability output as a security control.

Capabilities bound categories of authority; they do not prove that authorized
code is benign, authenticate network peers, constrain filesystem paths, or
replace dependency review.

### Builds and dependencies

Keel packages cannot define build scripts, and the compiler invokes Go builds
with `-trimpath` and `-buildvcs=false`. Conformance case 850 verifies
byte-identical binaries for its fixed input.

The stronger no-network claim is not yet true for every build. A program using
`std.sql` causes the driver to run `go mod tidy` to resolve
`modernc.org/sqlite`, which may contact the Go module proxy and writes temporary
module metadata. Treat SQL builds as host-toolchain dependency resolution until
that gap is closed.

Only local path dependencies exist in `keel.toml`; there is no Keel package
registry, lockfile, signature verification, or provenance service.

### Secrets

`std.config.Secret` makes configuration secrets a distinct type, but values can
be exposed explicitly with `unwrap()`. Keel does not provide a secret store,
rotation, process-isolation, or log-scrubbing guarantee. Keep unwrapped values at
the narrowest boundary and rely on deployment infrastructure for secret
delivery.

## Out of scope today

- security guarantees for a stable release or package registry;
- sandboxing generated executables at runtime;
- C ABI/FFI safety;
- cryptographic signing of toolchains or artifacts;
- multi-tenant compiler service isolation;
- formal verification of the compiler or runtime.

## Fix acceptance

Security fixes follow the same concern separation as other behavior changes:
decision/specification where semantics change, then conformance, then compiler
implementation. A fix must preserve stable diagnostic codes, deterministic
output, and all previously passing conformance cases. Embargo handling may make
the review private, but it does not lower the test requirement.
