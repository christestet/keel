# docs/spec/ — agent rules (adds to the root AGENTS.md, never replaces it)

- `keel-core.md` is **frozen** for M0–M4. Semantic changes require an issue
  (usually a KDR) first — there are no drive-by spec edits, even "obvious"
  ones. Typo and formatting fixes are fine; meaning changes are not.
- New chapters follow the numbering in `00-spec-plan.md` and land in their own
  PR that states which conformance tests will encode them. Tests follow in a
  second PR, implementation in a third (root AGENTS.md, hard rule 1).
- Every normative statement must be testable, and every error condition gets a
  stable `K####` code at spec-writing time — code examples in chapters will be
  extracted and run by CI (literate-spec discipline, see `00-spec-plan.md`).
- If spec prose conflicts with the conformance suite, never reconcile silently
  in either direction — file an issue (the prime directive).
