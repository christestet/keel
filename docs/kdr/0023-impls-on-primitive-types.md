# KDR-0023: User `impl` blocks on primitive types

- **Status:** proposed
- **Date:** 2026-06-18
- **Scope:** language

## Decision

A primitive built-in type (`Int`, `Float`, `Bool`, `String`, `Char`) may be the
target of an `impl Interface for <Primitive>` block, exactly like a user-defined
`struct` or `enum`. Such an impl supplies methods for **both** satisfaction
paths: nominal interface dispatch (KDR-0003, [`07-interfaces.md`](../spec/07-interfaces.md))
and structural generic-constraint satisfaction (KDR-0022,
[`08-generics.md`](../spec/08-generics.md)). A primitive satisfies an interface
bound through its `impl` blocks — there is **no** separate "intrinsic built-in
method" satisfaction facility, and primitives are **not** exempt from `impl`.
Within a single module a type (primitive or user-defined) may declare at most one
`impl` method of a given name, so structural method lookup is unambiguous.

## Context

[`KDR-0022`](0022-interface-constrained-generics.md) §1 defines constraint
satisfaction structurally: a method is available on `T` "if there exists an
`impl X for T` block (for any interface `X`) that declares a matching
signature," with no carve-out for primitives. The frozen conformance cases
`224-generic-struct` and `226-generic-method-call` write `impl Display for Int`,
`impl Stringer for String`, etc., and depend on them. The M5 implementation
follows both: structural satisfaction reads the type's `impl` blocks (boxing
primitives into Go wrapper types because Go forbids methods on predeclared
types — a codegen detail, see [`generics-implementation.md`](../generics-implementation.md)).

Spec [`08-generics.md`](../spec/08-generics.md) §8.3.1, however, states that
primitives satisfy bounds via "compiler-built-in methods" and that for a
primitive "no `impl` block is needed or **possible**." That sentence is not in
KDR-0022, contradicts the frozen conformance suite, and describes an
intrinsic-method facility that has never been implemented or tested. The
language never actually decided whether user code may write `impl I for Int`;
§8.3.1 asserted "no" while the tests and KDR-0022 assume "yes." This KDR settles
that question.

Prior art and its cost:

- **Go** has no `impl` keyword and forbids declaring methods on predeclared
  types (`int`, `string`) — you must define a named wrapper type. Cost:
  boilerplate wrappers; no coherence machinery needed.
- **Rust** allows `impl Trait for i32` but gates it with the orphan rule (trait
  or type must be local to the crate). Cost: a non-trivial coherence checker.
- **Swift** allows `extension Int: Protocol`. Cost: retroactive-conformance
  ambiguity across modules.

## Alternatives considered

- **Forbid `impl` on primitives; provide intrinsic built-in methods instead**
  (what §8.3.1 asserts). Rejected: it requires a built-in-method facility that
  does not exist and is on no roadmap; it contradicts frozen conformance cases
  `224`/`226`; and it forces a user who wants `Int: Display` to wrap `Int` in a
  newtype `struct` purely to attach a method — precisely the boilerplate
  KDR-0022 rejected when it chose structural (not nominal) constraint
  satisfaction.
- **Allow `impl` on primitives only within the type's defining module / behind
  an orphan rule.** Deferred, not rejected: Keel Core is single-module, so there
  is no second module that could supply a conflicting impl yet. A cross-module
  orphan/coherence rule is a real future need but premature before packages and
  stdlib land (M6/M7). This KDR scopes the decision to the single-module setting
  and explicitly leaves cross-module coherence to a follow-up KDR.
- **Allow `impl` on primitives unconditionally, single-module (this decision).**
  Chosen: it is the minimum that matches KDR-0022 and the conformance suite,
  adds no new syntax, and defers coherence machinery until modules exist to need
  it.

## Consequences

- `impl Interface for Int` (and the other four primitives) is well-formed; the
  methods serve both interface-value dispatch and generic-constraint
  satisfaction. No newtype wrapper is required at the source level.
- Spec §8.3.1 must be corrected in a separate spec PR (its own concern under
  root `AGENTS.md` hard rule 1): drop "no `impl` block is … possible"; state
  that primitives satisfy bounds through their `impl` blocks. Any future
  intrinsic-built-in-method satisfaction becomes an *additive* mechanism, not a
  precondition, consistent with KDR-0022 §2's "may optionally specialise."
- Backend: because Go cannot attach methods to `int64`/`string`/etc., primitive
  impls lower to boxed wrapper types (`keelBox_<Prim>`). This is a codegen
  consequence, not a language guarantee, and is invisible to Keel source.
- Coherence: at most one `impl` method of a given name per type keeps structural
  lookup deterministic and keeps generated Go free of duplicate methods. This
  already holds for structs/enums; the decision states it applies to primitives
  too. Cross-module coherence (two packages each providing `impl I for Int`) is
  out of scope and deferred to a packages-era KDR.
- No new keyword, no subtyping, no inheritance — reuses existing `impl` syntax
  and stays within KDR-0003's nominal-dispatch model.

## Reopening clause  *(required)*

Reopen if **either** of the following is demonstrated:

1. **Coherence harm in the multi-module setting.** Once packages exist, corpus
   evidence from at least three distinct Keel codebases each exceeding 10,000
   lines that unrestricted (single-module-style) primitive impls cause a
   measured incoherence bug class — e.g. two dependencies providing
   behaviourally-incompatible `impl I for Int` whose effective method silently
   depends on link/import order — that a module-local orphan rule could not
   prevent without also disallowing the single-module primitive impls this KDR
   permits. (A follow-up KDR adding an orphan rule for the cross-module case is
   *not* a reopening of this decision; it is the deferred work named above.)
2. **Codegen cost.** Reproducible benchmark evidence that the boxed-wrapper
   lowering imposes a runtime or binary-size regression beyond an agreed bound
   (to be fixed in the proposal, e.g. > 5 % on a representative workload) with no
   available backend workaround.

Advocacy, aesthetic preference, and "language X forbids/permits it" are never
sufficient.
