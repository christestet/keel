# tests/conformance/ — agent rules (adds to the root AGENTS.md, never replaces it)

This directory is the executable spec. `README.md` here is normative for the
case format and numbering; this file is only what agents actually break.

- **Never add a case for behavior not in `docs/spec/keel-core.md`** (or a later
  ratified spec chapter). If the program needs an unspecified feature, the case
  is premature — stop and open an issue instead.
- **Never edit an existing `expected.stdout` / `expected.error` to make the
  compiler pass.** That is spec drift, the exact failure mode this repo is
  built to prevent. If you believe an expectation is wrong, open an issue.
- Validate before committing: `cargo run -p conformance-runner -- --check`.
