# compiler/ — agent rules (adds to the root AGENTS.md, never replaces it)

Read `ARCHITECTURE.md` in this directory before touching any crate. Its iron
rules are review-blocking; the ones agents break most often:

- **No panics on user input.** No `unwrap()`/`expect()`/raw indexing on anything
  derived from source text. Malformed source produces `K####` diagnostics.
- **Determinism.** Never iterate a `HashMap`/`HashSet` into output (diagnostics
  order, generated Go, formatter output). Sort first, or use ordered structures.
- **The formatter is the AST pretty-printer.** There is no second formatting
  code path — do not create one.
- **No new dependencies** without a justifying PR. Check your `Cargo.toml` diff
  before committing; transitive additions count.

Structure rules:

- New crates follow the `keelc-<stage>` layout in `ARCHITECTURE.md`. A crate
  not listed there needs an issue first, not a drive-by addition.
- Every stage is a salsa-style memoized query keyed on inputs. Do not bolt side
  effects (I/O, global state) onto query functions.
- New diagnostics: register the next free `K####` in the `keelc-diag` registry
  file (append-only — never reuse or renumber), then encode the behavior in a
  conformance reject-case in a *separate* PR.

Scope: stay inside the current `ROADMAP.md` milestone — e.g. no backend code
during M2. Done means `scripts/preflight.sh` is green from the repo root and
the PR description lists the conformance cases the change makes pass.
