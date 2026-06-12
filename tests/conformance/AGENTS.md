# tests/conformance/ — agent rules (adds to the root AGENTS.md, never replaces it)

This directory is the executable spec. `README.md` here is normative for the
case format; this file is the short list of rules agents actually break.

- **Never add a case for behavior not in `docs/spec/keel-core.md`** (or a later
  ratified spec chapter). If the program needs an unspecified feature, the case
  is premature — stop and open an issue instead.
- **Never edit an existing `expected.stdout` / `expected.error` to make the
  compiler pass.** That is spec drift, the exact failure mode this repo is
  built to prevent. If you believe an expectation is wrong, open an issue.
- One behavior per case. A case that tests two things is two cases.
- Reject-cases are minimal: removing any line must make the error disappear.
- Case numbers are permanent. Take the next free number in the correct band
  (see `README.md`); gaps are fine, renumbering is forbidden.
- Reject-cases match stable `K####` codes (registered in `keelc-diag`), never
  message text.
- Validate before committing: `cargo run -p conformance-runner -- --check`.
- Conformance changes are their own PR — never mixed with spec or compiler
  changes (root AGENTS.md, hard rule 1).
