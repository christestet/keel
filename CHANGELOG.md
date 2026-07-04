# Changelog

This file records user-visible changes from the point it was introduced.
Milestone history prior to the first tagged release is not retroactively
presented as released versions; use
[`docs/milestone-status.md`](docs/milestone-status.md) and Git history for that
development record.

## Unreleased

- `keel build|run|fmt|test|check|audit` now default `--milestone` to the
  latest implemented milestone (M7) instead of M1, so running any file
  command without the flag exercises the complete current language rather
  than the parser-only gate. The flag remains available to check a program
  against an earlier milestone's gate.
- Added a `Dockerfile` (`docker build -t keel . && docker run --rm keel`) so
  the developer preview can be tried with no local Rust/Go install.

## [0.1.1](https://github.com/christestet/keel/compare/v0.1.0...v0.1.1) (2026-07-04)


### Bug Fixes

* **docs:** enable hidden files in pages artifact for Jekyll cache compatibility ([#39](https://github.com/christestet/keel/issues/39)) ([f864e2c](https://github.com/christestet/keel/commit/f864e2c407bcc449f39bfb00cf1f0556f38c23d0))

## [0.1.0] — 2026-07-03 (developer preview)

First tagged version. A **developer preview**, not production infrastructure:
the gate and honest limitations are
[`docs/0.1-release-readiness.md`](docs/0.1-release-readiness.md).

### Language and toolchain

- Full M7 language surface: Core (functions, structs, enums, exhaustive
  `match`, `Option`/`Result`/`?`/`catch`, union errors), interfaces,
  constrained generics, `scope`/`spawn` structured concurrency, arenas
  (tail escape check), packages with manifests and capabilities, editions
  (edition 1), stdlib slice (`std.http`, `std.json`, `std.sql` SQLite,
  `std.log`, `std.config`, `std.time`, boundary scalars).
- Tool commands: `build`, `run`, `fmt`, `test`, `check`, `audit`,
  proto3-subset `gen`, `lsp` (M8 base capabilities, module-level symbol
  resolution only), and `--version`/`-V` reporting version + build commit.
- `keel audit` now reports an implicit (manifest-less) package's **derived**
  capability set (`net: self (derived)`) instead of `not present`
  (KDR-0043; conformance case 829).
- Compile-time contract (KDR-0019) enforced in CI on the reference machine:
  `keel check` 228 ms (budget 300), cold build 9451 ms (budget 10 000);
  `keel_build_incremental` remains a documented known gap (1121 ms vs 1000).
- Release artifacts: tag-triggered workflow builds Linux x86_64 and macOS
  arm64 tarballs with SHA-256 checksums, attached to a draft GitHub release.

### Documentation

- Source-build onboarding, conformance-backed language tour, CLI,
  standard-library, diagnostics, package/capability, deployment,
  troubleshooting, compatibility, security, syntax, and feature-status
  references; positioning and idiomatic Keel guidance.
- Spec chapters 02 (types), 03 (declarations), and 05 (errors) authored as
  consolidations of conformance-backed behavior; chapters 12 (FFI) and 13
  (testing) remain unauthored.
- Release-tarball install path documented for macOS/Linux.

### Known limitations

No package registry, no native backend (Go toolchain required), no OCI
output, no FFI, no `keel fix`; LSP resolves module-level declarations only;
`std.sql` builds may resolve `modernc.org/sqlite` via the Go module proxy.
See [`docs/feature-status.md`](docs/feature-status.md).

### Validation snapshot (release commit)

```text
scripts/preflight.sh                       → green (91 passed, 0 failed, 135 skipped)
KEEL_MILESTONE=M7 scripts/preflight.sh     → green (221 passed, 0 failed, 5 skipped;
                                             the 5th skip is the M8-gated case 829)
KEEL_MILESTONE=M8 conformance-runner       → 222 passed, 0 failed, 4 skipped
M1–M6 gates                                → 91/91/95/97/122/194 passed, 0 failed
scripts/m8-benchmark.sh --mode full --enforce --known-gap keel_build_incremental
  (local Apple Silicon; canonical numbers are the CI reference machine's)
  keel_check 115 ok · keel_build_cold 2741 ok · keel_build_incremental 573 ok
  CI reference machine (run 28676356124): 228 / 9451 / 1121
SQL cases resolved modernc.org/sqlite from the warm local Go module cache.
```

## Changelog policy

Every release moves applicable entries from `Unreleased` into a versioned
section with its release date. Entries describe observable user impact, not the
internal commit sequence. Breaking changes must name the affected edition,
migration command/status, and governing KDR.
