# Generics implementation status

Non-normative implementation tracking for interface-constrained generics (M5,
[`docs/spec/08-generics.md`](spec/08-generics.md)).

## Status — end-to-end (typechecker + backend) complete

All eleven generics conformance cases (`223`–`233`) pass at `M5`. The approach is
**type erasure to the bound interface** with **primitive boxing**: a type
parameter `T: I` lowers to the Go interface `I`, structs satisfy it through their
existing receiver methods, and primitive `impl`s (which Go cannot attach to
predeclared types) are emitted as `keelBox_<Prim>` wrapper types carrying every
impl method. No monomorphization or dictionary passing is required.

### Done (this milestone)

- **Type representation:** `TypeInfo::TypeParam { name, bound }` in `keelc-types`,
  with `type_param_bounds` / `substitute_type_params` helpers. Type parameters are
  kept distinct through checking and erased to the bound interface only at Go
  emission.
- **Typechecker (`keelc-resolve`):** `check_function` substitutes `Named(T)` →
  `TypeParam`; `infer_method_call` emits `K0802` when a body calls a method outside
  the bound; `check_assignable` performs structural constraint satisfaction and
  emits `K0803` (with the missing method names) when a concrete type argument lacks
  a bound method. Struct field access now resolves field types so methods can be
  called on generic-struct fields.
- **KIR lowering (`keelc-kir`):** `lower_function` / `lower_struct_decl` forward
  the substituted `TypeParam` types into KIR params, returns, and fields.
- **Go backend (`keelc-backend-go`):** `go_type`/`zero_value` erase `TypeParam` to
  its bound interface; `keelBox_<Prim>` wrapper types + methods are emitted for
  primitive impls; primitive arguments are boxed when flowing into interface /
  type-parameter slots at call sites and struct literals.
- **Parser:** `K0806` now points at the type-parameter `[` (so reject-case `232`
  matches `line:1`).

### Done (parser scaffolding, earlier)

- **Diagnostic registry:** `K0801`–`K0807` registered in
  `compiler/keelc-diag/src/registry.rs`. Parser emits `K0801` (missing bound),
  `K0804` (duplicate name), `K0805` (shadows built-in), `K0806` (too many params).
- **AST types** (`compiler/keelc-ast/src/lib.rs`): `TypeParam` struct with `name`,
  `bound: Option<Spanned<String>>`, `span`. `type_params` on `FunctionDecl`,
  `StructDecl`, `EnumDecl`. `type_args` on `Expr::Call`, `Expr::StructLiteral`,
  `ImplDecl`.
- **Pretty printer** (`compiler/keelc-ast/src/pretty.rs`): Prints type params
  `[T: Bound]` on functions, structs, enums, impls; prints type args `[T, U]` on
  calls and struct literals.
- **Parser** (`compiler/keelc-parse/src/lib.rs`):
  - `parse_type_params()` — parses `[T: Interface, ...]`, M5-gated
  - `parse_type_args_in_brackets()` — parses `[T, U]`, M5-gated
  - Function decls: `fn foo[T: Bound](x: T)`
  - Struct decls: `struct Pair[A: Bound, B: Bound]`
  - Enum decls: `enum Result[T: Bound, E: Bound]`
  - Impl headers: `impl Interface for Type[A, B]`
  - Calls: `foo[T, U](args)`
  - Struct literals: `Pair[Int, String]{ ... }`
- **Conformance fix:** test 233 `expected.error` changed `K0807` → `K0601` (subsumed);
  spec note added about overlap.

### Explicitly not done (beyond the conformance surface)

- **`K0807`** (interface bound declares >5 methods) stays a reserved safety net,
  subsumed by `K0601` at the interface declaration site (case `233` asserts
  `K0601`). It is not separately emitted.
- **Type-argument inference from struct-literal fields:** `Pair { ... }` without
  explicit `[Int, String]` is not inferred; explicit type args are used. Inference
  is implemented only for function-call value arguments (Go-side erasure makes the
  explicit args inert at runtime).
- **Generic `impl` blocks** (`impl I for Pair[A, B]`) and generic enums are parsed
  but not exercised by conformance; no method-body type-parameter scope is set up
  for them yet.
- **`expr_ty(Name)` is `Unknown` in the backend**, so boxing a *variable* of
  primitive type into an interface slot is not yet handled — every conformance case
  passes literals. Revisit when a case needs it.

## Dependency chain

- [`docs/spec/08-generics.md`](spec/08-generics.md) — normative spec
- [`docs/kdr/0022-interface-constrained-generics.md`](kdr/0022-interface-constrained-generics.md) — design decisions
- [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) — pipeline, crate layout
- [`tests/conformance/223-*`](../tests/conformance/223-generic-function/) through
  [`tests/conformance/233-*`](../tests/conformance/233-constraint-interface-too-many-methods/) — 11 M5-gated cases

## Milestone boundary

`ROADMAP.md` puts generics in M5. The parser scaffolding is M5-gated (defaults to M1,
enabled at milestone ≥5). All existing M0–M4 tests pass unchanged at M1.

## Validation snapshot

```
$ KEEL_MILESTONE=M5 scripts/preflight.sh
...
114 passed, 0 failed, 1 skipped
preflight: green
```

At the runner's default (M1) gate, generics and interface cases remain skipped:

```
$ cargo run -p conformance-runner -- --keelc target/release/keelc
89 passed, 0 failed, 26 skipped
```

## Next work

- **`scope` / `spawn` structured concurrency** (chapter 09) is the remaining M5
  language-completion item; generics are complete.
- Optional hardening (not blocking M5): type-argument inference for generic struct
  literals, generic `impl`/enum bodies, and backend boxing of primitive *variables*
  (see "Explicitly not done").
