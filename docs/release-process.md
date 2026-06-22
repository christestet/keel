# Release process

Keel has not published a release. This process defines the minimum bar for the
first source release without pretending binary distribution, signing, support
windows, or package infrastructure already exists.

## 1. Confirm release scope

- The release corresponds to a completed roadmap milestone with a binary exit
  criterion.
- Every included behavior has the required decision/spec/conformance/compiler
  sequence.
- No proposed KDR is presented as accepted behavior.
- Trigger-gated and future features remain clearly marked as unavailable.

Release preparation is its own concern. Do not combine unrelated language,
conformance, compiler, or harness changes into the release commit.

## 2. Choose the version

Before 1.0, use `0.MINOR.PATCH`:

- increment `MINOR` for a completed feature milestone or intentional language
  surface addition;
- increment `PATCH` for fixes/documentation that preserve the conformance-backed
  surface.

The repository currently contains Cargo package version `0.1.0`, but that value
has not been published and is not evidence of a release.

Update every workspace crate version consistently or document why an internal
crate differs. Package-manifest versions in conformance fixtures are test data
and are not release versions.

## 3. Freeze observable inputs

Record:

- release commit and proposed tag;
- Rust and Go toolchain versions;
- host OS/architecture used for validation;
- active language edition(s);
- conformance count and intentional skips;
- known implementation/specification gaps from
  [feature status](feature-status.md).

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

Any failure blocks the release. Do not alter a conformance expectation to make
the release pass.

## 5. Update release documentation

- Move relevant `CHANGELOG.md` entries from `Unreleased` to
  `## [VERSION] — YYYY-MM-DD`.
- Update `README.md`, [feature status](feature-status.md), milestone status, and
  compatibility/support statements to the exact released state.
- Verify all CLI examples against the release binary.
- List known limitations prominently; roadmap items are not release features.
- Ensure `SECURITY.md` names a working private reporting route before inviting
  external production use.

## 6. Build artifacts

The current supported deliverable is source. The repository has no automated,
cross-platform binary release pipeline, checksums/signing policy, SBOM output,
or OCI publisher. Do not manually upload an ad-hoc binary and call it an
official supported toolchain.

Binary releases require a separate accepted process covering:

- supported targets and build hosts;
- reproducible release builders;
- cryptographic checksums/signatures and key ownership;
- provenance/SBOM generation;
- installation and upgrade paths;
- security backport lifetime.

M9 owns reproducible OCI images; M11 owns removal of the Go backend dependency.

## 7. Tag and announce

After the release commit is reviewed and validation evidence is attached:

```sh
git tag -a vVERSION -m "Keel vVERSION"
```

Tagging/pushing is a maintainer action. The announcement links the changelog,
compatibility policy, security policy, installation instructions, and exact
validation summary. It distinguishes implemented behavior from roadmap work.

## 8. After release

- Keep the release tag immutable.
- Restore an empty `Unreleased` section in the changelog.
- Triage regressions against the released commit and smallest reproducer.
- For a security issue, follow the private process in `SECURITY.md`; revoke or
  replace artifacts rather than mutating a published tag.
- Do not promise backports until a support-window policy has been accepted.
