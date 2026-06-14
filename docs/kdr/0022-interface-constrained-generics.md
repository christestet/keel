# KDR-0022: Interface-constrained generics

- **Status:** proposed
- **Date:** 2026-06-14
- **Scope:** language

## Decision

User-defined generics use parametric polymorphism with interface constraints.
Constraints are **structurally** satisfied (method-set matching), while runtime
interface dispatch remains **nominal** (explicit `impl` blocks per KDR-0003).

### 1. Constraint form and checking

Every type parameter must be bounded by an explicit interface — no unconstrained
parameters. Constraint satisfaction is structural: a type `T` satisfies
`interface Foo` at a generic instantiation site iff `T` has methods matching
every signature declared in `Foo`. No `impl Foo for T` block is required for
constraint satisfaction.

An explicit `impl Foo for T` block **is** required when a value of type `T` is
used at a position expecting the interface type `Foo` (runtime dispatch, per
KDR-0003). The two satisfaction paths are separate:

| Context | Satisfaction model | Why |
|---|---|---|
| Generic constraint `fn f[T: Foo](x: T)` | Structural — method-set match | Compile-time only; dictionary is passed by the caller |
| Interface value `fn f(x: Foo)` | Nominal — explicit `impl` block | Runtime dispatch needs a known vtable layout |

Constraints are simple method sets. There are no union types, no type-set
algebra, no `|` operator in constraint position. This avoids the "core types"
complexity that plagued Go's initial generics design (resolved in Go 1.25 by
removing the concept entirely).

### 2. Implementation strategy

Implementation uses **dictionary passing** (not monomorphization):

- A generic function receives a method dictionary (vtable) for each
  interface-constrained type parameter at the call site.
- The generic function body is compiled once; calls to constrained methods are
  dispatched through the dictionary.
- This preserves separate compilation — modules can be compiled independently
  without knowing all concrete instantiations.
- Code size is independent of the number of concrete types.

For primitive types (`Int`, `Float`, `Bool`, `String`, `Char`), the compiler
may optionally specialize (inline + monomorphize) known instantiations for
performance. This is a backend optimisation, not a language-level guarantee.

### 3. Syntax

Generic functions and types are declared with explicit type parameters in angle
brackets, matching the syntax of existing built-in generics (`Option<T>`,
`Result<T, E>`):

```keel
fn first[T: Stringer](items: List<T>) -> T { ... }

struct Pair[A: Stringer, B: Stringer] {
    first: A
    second: B
}
```

The built-in generic types (`Option<T>`, `Result<T, E>`, `List<T>`, `Map<K, V>`)
continue to be compiler-built-in; user generics follow the same syntax with
explicit interface bounds.

## Context

Keel already has nominal interfaces with explicit `impl` blocks (KDR-0003,
[`docs/spec/07-interfaces.md`](../spec/07-interfaces.md)). The M5 roadmap
requires user-defined generics, and "interface-constrained" is already stated
in [`ROADMAP.md`](../../ROADMAP.md).

Two mechanisms live side by side because they solve different problems:

- **Runtime dispatch** (interface values) needs a nominal `impl` block so the
  compiler can construct a fixed-layout vtable and so that `impl` relationships
  are grepable and explicit (KDR-0003's requirement).
- **Compile-time constraints** (generics) are satisfied structurally because
  the constraint is checked once per call site and the implementation uses
  dictionary passing — no vtable layout needs to be fixed at the type-definition
  site.

This dual model is proven in practice: Go uses structural satisfaction for both
generics and interface dispatch with no `impl` keyword. Keel adds nominal
`impl` blocks only for the runtime path (KDR-0003), while keeping the simpler
structural model for generic constraints where the explicitness cost outweighs
the benefit.

The decision against monomorphization is based on separate compilation: when
two independently-compiled modules use `List<MyType>`, monomorphization would
require either (a) compiling the generic body once per module (code bloat), or
(b) a linker-level merging step (complexity). Dictionary passing avoids both.
For a backend-service language where compilation speed and binary size matter
more than peak single-instantiation throughput, dictionary passing is the
correct default.

## Alternatives considered

- **Nominal constraint satisfaction** (require `impl Interface for Type` for
  generic constraints). Rejected: this would force users to write `impl` blocks
  for types that already have the right methods, purely to satisfy the
  constraint checker. The cost in boilerplate outweighs the explicitness
  benefit — generic constraints are checked at the instantiation site, not
  discovered through grep.

- **Monomorphization** (Rust/C++ model — compile a copy per concrete type).
  Rejected: breaks separate compilation, increases code size, slows
  compilation. Keel targets containerised backend services where binary size
  and iteration speed matter more than raw throughput.

- **Union type constraints** (`T: InterfaceA | InterfaceB`). Rejected: this
  creates the "core types" complexity that Go 1.18 introduced and Go 1.25
  removed. Constraints are simple method sets; no type-set algebra.

- **Java/C# bounded generics** (`<T extends Interface>`). Rejected: these are
  subtype-bounded quantification, inheritance-adjacent, and reintroduce
  subtyping relationships KDR-0003 excluded.

- **C++ templates** (unconstrained, duck-typed). Rejected: unconstrained
  parameters produce diagnostics at the instantiation site rather than the
  definition site, violating Keel's "illegal states unrepresentable" principle.

- **Higher-kinded polymorphism.** Rejected: adds significant type-system
  complexity for marginal benefit to the backend-service target.

- **No user-defined generics.** Rejected: M5 exit criterion requires them per
  [`ROADMAP.md`](../../ROADMAP.md).

## Consequences

- Every generic function or type must declare explicit interface bounds for its
  type parameters. No unconstrained `fn foo[T](x: T)`.
- Constraint satisfaction is structural — a type with the right methods
  automatically satisfies a bound. No `impl` block needed for generics.
- Runtime interface dispatch remains nominal — explicit `impl` blocks required
  per KDR-0003.
- Dictionary passing keeps code size small and separate compilation intact.
  Primitives may be specialised as a backend optimisation.
- Well-formedness diagnostics (using a method not in the bound) fire at the
  generic definition site. Satisfaction diagnostics (type missing a required
  method) fire at the instantiation site.
- Code that would use unconstrained generics in Rust or Go must introduce a
  bound in Keel. For the common case of "any type with these methods," the
  bound is an interface the author or stdlib already defines.
- The current diagnostic `K0901` ("user-defined generics are not in Core") will
  be reused for actual generics-related diagnostics when implementation lands.
  The conformance case `901-no-user-generics` will be superseded by the new
  generics conformance suite.
- No new keyword, no subtyping dimension, no diamond problem. Generics reuse
  the existing interface syntax with structural satisfaction for constraints.

## Reopening clause

Corpus evidence — from at least three distinct Keel codebases exceeding 10,000
lines each — that the structural-constraint + dictionary-passing model excludes
a significant class of real-world backend patterns that would demonstrably be
better served by an alternative generics design, AND that no practical
workaround exists via concrete types, manual duplication, or `keel gen` code
generation. "Better served" must be measured by metric: lines of code,
compilation time, or runtime performance, with a concrete bound on acceptable
regression.
