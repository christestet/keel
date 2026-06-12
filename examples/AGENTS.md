# examples/ — agent rules (adds to the root AGENTS.md, never replaces it)

- `hello.keel` must stay within Keel Core (`docs/spec/keel-core.md`) — it is
  the M3 exit criterion.
- `users-service/` is **aspirational** (the M6 exit criterion). It deliberately
  uses post-Core features. Do not "fix" it to compile under Core; Core grows to
  meet it (see its README).
- Examples are not tests. Behavior guarantees live in `tests/conformance/`;
  do not add expected-output files here.
- A new example must state which milestone it targets (README or header
  comment) and use no language features beyond that milestone.
