# Deploying current Keel programs

This guide describes the M7 implementation, not the complete deployment design
in `vision.md`. The current compiler emits a host executable through Go. It does
not build OCI images, cross-compile through a Keel flag, generate SBOMs, expose
runtime profiles, or provide deployment manifests.

## Build the executable

```sh
target/release/keel build service/main.keel --milestone M7
```

The artifact is written beside the source and named after its stem. The example
above produces `service/main` (plus the platform executable suffix).

The driver invokes Go with `-trimpath` and `-buildvcs=false`. Conformance case
850 verifies byte-identical output for a fixed Core program built twice in the
same toolchain environment. It does not prove cross-host reproducibility for
every standard-library combination.

## Host requirements

Building currently requires the Go toolchain. The produced executable targets
the build host because `keel build` has no target-selection interface.

Programs importing `std.sql` bundle the pure-Go `modernc.org/sqlite` driver, but
the compiler first runs `go mod tidy`; the module must already be cached or the
build host needs Go module proxy access.

Verify the produced artifact on every target platform rather than assuming it
is static or cross-platform. The driver does not force `CGO_ENABLED=0` or expose
`GOOS`/`GOARCH` configuration as a Keel contract.

## Configuration

`std.config` reads environment variables derived from struct fields:

```keel
use std.config

struct AppConfig {
    database_url: Secret
    port: Int = 8080
}
```

`database_url` reads `DATABASE_URL`; `port` reads `PORT` and uses `8080` when
absent. Supply required variables through the process supervisor or container
runtime. `Secret` prevents accidental type confusion but is not a secret store;
`unwrap()` exposes the underlying string.

## HTTP services

`http.serve` binds the requested port and blocks. A bind failure is returned as
`http.Error`; a port outside 1–65535 panics with `K1505`.

The current Go runtime uses `http.ListenAndServe` directly. It does not install
a graceful-shutdown handler, readiness/health endpoints, TLS configuration, or
OpenTelemetry instrumentation. Implement endpoints in application code and let
the process supervisor terminate the process; do not claim graceful drain until
the runtime has conformance coverage for it.

## SQLite data

SQLite connection strings use `:memory:` for ephemeral state or a filesystem
data-source name for persistent state. A package using `std.sql` must declare
both `net` and `fs`, even for a known SQLite-only service, because the normative
capability map is conservative.

Use an external, versioned migration system for production schema management.
`pool.migrate()` is a sequential statement runner intended for examples and
tests; it has no migration history, locking, rollback, or retry protocol.

## Minimal container shape

Keel does not yet emit or test container images. If the host executable is
verified to have no dynamic-library requirements for the target, it can be
placed in a minimal image manually. That manual image is outside the current
hermetic/reproducibility guarantee.

Do not assume CA certificates, timezone data, user/group entries, or writable
directories exist in `FROM scratch`. Add only the runtime files the service
actually needs and run as a non-root UID where the deployment platform permits.

M9 owns daemonless, byte-identical OCI output. Until it lands, Dockerfiles,
Helm charts, and image digests are application infrastructure rather than Keel
toolchain artifacts.

## Pre-deployment checks

1. Run `keel check`, `keel test`, and `keel build` with `--milestone M7`.
2. Run `keel audit` against an explicit package manifest.
3. Verify the binary on the actual target OS/architecture.
4. Exercise startup failure, port conflicts, missing configuration, database
   persistence, and process termination.
5. Record the compiler commit and Go toolchain version used for the build.

See [Security](../SECURITY.md) for current trust boundaries and
[Packages and capabilities](packages-and-capabilities.md) for audit semantics.
