---
description: Scaffold a new conformance case in tests/conformance/
argument-hint: behavior to encode, e.g. "reject non-exhaustive match on enum with payload"
---
Create a new conformance case for: $ARGUMENTS

Follow `tests/conformance/AGENTS.md` and `tests/conformance/README.md` exactly:

1. Verify the behavior is specified in `docs/spec/keel-core.md` (cite the
   section in your report). If it is not specified, STOP and say the case is
   premature — never invent language features.
2. Pick the correct number band from the README (`0xx` lexical … `9xx`
   not-in-Core rejections) and take the next free number in that band.
3. Create `NNN-kebab-name/main.keel` plus exactly one of `expected.stdout`
   (accept) or `expected.error` (reject; first line `K####`, optional second
   line `line:N`). Add `case.toml` with a `milestone = "MN"` gate if the
   behavior lands after the current milestone.
4. Reject-cases must be minimal: removing any single line of `main.keel` must
   make the error disappear. The `K####` code must exist in the keelc-diag
   registry (or be registered via the proper compiler-PR flow first).
5. Validate: `cargo run -p conformance-runner -- --check`.

Remember: conformance changes are their own PR — never mixed with spec or
compiler changes.
