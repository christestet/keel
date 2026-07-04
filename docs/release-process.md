# Release process

Keel has not published a release. This process defines the minimum bar for the
first source release without pretending binary distribution, signing, support
windows, or package infrastructure already exists.

For the first public developer-preview release, also apply the explicit
[`0.1.0 release readiness`](0.1-release-readiness.md) gate. A source-only
checkpoint and a usable public toolchain release are different claims.

## 1. Confirm release scope

- The release corresponds to a completed roadmap milestone with a binary exit
  criterion.
- Every included behavior has the required decision/spec/conformance/compiler
  sequence.
- No proposed KDR is presented as accepted behavior.
- Trigger-gated and future features remain clearly marked as unavailable.
- The release announcement states whether this is a source-only checkpoint or a
  public developer preview with supported installation artifacts.

Release preparation is its own concern. Do not combine unrelated language,
conformance, compiler, or harness changes into the release commit.

## 2. Choose the version

`release-please` (`.github/workflows/release-please.yml`,
`release-please-config.json`) does this mechanically from
[Conventional Commits](https://www.conventionalcommits.org/) merged to `main`
since the last tag, and keeps every workspace crate version and
`CHANGELOG.md` in sync via a standing `chore(main): release X.Y.Z` PR. Before
1.0 it treats `feat` as `MINOR` and `fix`/`docs`/etc. as `PATCH`:

- `MINOR` for a completed feature milestone or intentional language surface
  addition;
- `PATCH` for fixes/documentation that preserve the conformance-backed
  surface.

Review the proposed version and changelog for accuracy before merging; a
mis-typed commit type produces a wrong bump. Package-manifest versions in
conformance fixtures are test data and are not release versions.

## 3. Freeze observable inputs

Record:

- release commit and proposed tag;
- Rust and Go toolchain versions;
- host OS/architecture used for validation;
- active language edition(s);
- conformance count and intentional skips;
- known implementation/specification gaps from
  [feature status](feature-status.md).
- for 0.1.0, the current checklist state from
  [0.1.0 release readiness](0.1-release-readiness.md).

Do not claim support for an untested host/target.

## 4. Run validation

From a clean checkout, run the executable definition of done at M7 (or the
current release milestone):

```sh
KEEL_MILESTONE=M7 scripts/preflight.sh
```

Then verify earlier gates explicitly because milestone-bounded rejection cases
are intentionally skipped at later milestones:

```sh
for milestone in M1 M2 M3 M4 M5 M6 M7; do
    KEEL_MILESTONE="$milestone" scripts/preflight.sh
done
```

Run documentation relative-link checks and execute every getting-started command
from a clean working directory. SQL conformance may require an approved Go
module cache/network path for `modernc.org/sqlite`; record which was used.

For a public 0.1.0 developer preview, also run the M8 performance gate from the
published reference machine:

```sh
scripts/m8-benchmark.sh --mode full --enforce
```

The benchmark must use nonzero checked-in baselines; zero baselines are useful
for fixture development only and cannot justify a release claim.

Any failure blocks the release. Do not alter a conformance expectation to make
the release pass.

## 5. Update release documentation

- `release-please` moves `CHANGELOG.md` entries out of `Unreleased` into
  `## [VERSION] — YYYY-MM-DD` as part of its release PR; review the generated
  section for accuracy rather than writing it by hand.
- Update `README.md`, [feature status](feature-status.md), milestone status, and
  compatibility/support statements to the exact released state.
- Verify all CLI examples against the release binary.
- List known limitations prominently; roadmap items are not release features.
- Link [0.1.0 release readiness](0.1-release-readiness.md) from the release
  notes while 0.1.0 is still unreleased.
- Ensure `SECURITY.md` names a working private reporting route before inviting
  external production use.

## 6. Build artifacts

`.github/workflows/release.yml` is the only sanctioned artifact path: a `v*`
tag builds `keel`/`keelc` for Linux x86_64 and macOS arm64 with the release
commit embedded (verified against `keel --version` in the job), packages
tarballs with SHA-256 checksums, and attaches them to a **draft** GitHub
release. Publishing the draft is a human decision gated on this process and
[0.1.0 release readiness](0.1-release-readiness.md). Do not manually upload an
ad-hoc binary and call it an official supported toolchain.

Still missing, and required before anything stronger than a developer preview
is claimed:

- cryptographic signatures (incl. macOS notarization) and key ownership;
- provenance/SBOM generation;
- upgrade paths and security backport lifetime;
- an OCI publisher (M9).

M9 owns reproducible OCI images; M11 owns removal of the Go backend dependency.

## 7. Tag and announce

After the release commit is reviewed and validation evidence is attached,
merge the standing `chore(main): release X.Y.Z` PR that `release-please`
opened. Merging pushes the `vVERSION` tag directly (it does not create a
GitHub Release itself), which triggers `release.yml` to build binaries and
open the DRAFT GitHub release. Publishing that draft remains a manual,
reviewed step — CI never announces a release on its own. The announcement
links the changelog, compatibility policy, security policy, installation
instructions, and exact validation summary. It distinguishes implemented
behavior from roadmap work.

## 8. After release

- Keep the release tag immutable.
- Restore an empty `Unreleased` section in the changelog.
- Triage regressions against the released commit and smallest reproducer.
- For a security issue, follow the private process in `SECURITY.md`; revoke or
  replace artifacts rather than mutating a published tag.
- Do not promise backports until a support-window policy has been accepted.
