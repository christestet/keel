# Changelog

This file records user-visible changes from the point it was introduced. Keel
has no published release yet, so earlier milestone history is not retroactively
presented as released versions; use
[`docs/milestone-status.md`](docs/milestone-status.md) and Git history for that
development record.

## Unreleased

### Documentation

- Added source-build onboarding and a conformance-backed language tour.
- Added current CLI, standard-library, diagnostics, package/capability,
  deployment, troubleshooting, compatibility, security, syntax, and feature
  status references.
- Added explicit positioning and idiomatic Keel guidance.
- Corrected public status from stale M6 claims to the completed M7 gate.
- Documented implementation/specification gaps instead of presenting planned
  behavior as available.
- Added an explicit 0.1.0 release-readiness gate and linked it from public
  roadmap, status, compatibility, and release-process docs.

### Current implementation baseline

- M7 conformance gate: 221 passed, 0 failed, 4 intentionally skipped
  earlier-milestone rejection cases.
- Implemented tool commands: `build`, `run`, `fmt`, `test`, `check`, `audit`,
  and proto3-subset `gen`.
- Executable generation still uses the Go backend; M8 is in progress and
  M9–M11 remain roadmap work.

## Changelog policy

Every release moves applicable entries from `Unreleased` into a versioned
section with its release date. Entries describe observable user impact, not the
internal commit sequence. Breaking changes must name the affected edition,
migration command/status, and governing KDR.
