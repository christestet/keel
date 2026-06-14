# Generics implementation status

Non-normative implementation tracking for interface-constrained generics (M5,
[`docs/spec/08-generics.md`](spec/08-generics.md)).

## Status — parser scaffolding complete

### Done

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

### Explicitly not done

- **Typechecker** (`compiler/keelc-resolve`): generic function resolution, type
  argument inference at call sites, bound satisfaction checking (K0802, K0803, K0807).
  No type param scope tracking during resolution.
- **KIR lowering** (`compiler/keelc-kir`): `type_params`/`type_args` not forwarded
  from AST to KIR declarations.
- **Go backend** (`compiler/keelc-backend-go`): dictionary passing for generic
  functions (erase params to bound interface types), monomorphization for generic
  structs, `impl` dispatch for generic impls.
- **Type inference:** `Pair{...}` without explicit `[Int, String]` type args does
  not infer from field types for generic structs.

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
$ cargo test --workspace && cargo run -p conformance-runner -- --keelc target/release/keelc
suite ok: 115 case(s)
89 passed, 0 failed, 26 skipped
```

All 11 generics cases skipped at M1 (gated to M5).

## Next work

1. **Typechecker** — `keelc-resolve`:
   - Track type param declarations in function/struct/enum scope
   - At call sites, infer type args from value arguments
   - Check bound satisfaction structurally (K0803)
   - Check method access against bounds (K0802)
2. **KIR lowering** — `keelc-kir/src/lower.rs`:
   - Forward `type_params` from AST `FunctionDecl`, `StructDecl`, `EnumDecl` to
     KIR equivalents
   - Forward `type_args` from `Expr::Call`, `Expr::StructLiteral` to KIR expressions
3. **Go backend** — `keelc-backend-go`:
   - Generic functions: erase type params to bound interface types in Go signatures,
     leverage Go's native interface vtable dispatch
   - Generic structs: monomorphize (rename per instantiation)
   - Generic impls: emit standalone Go functions for primitive type impls
4. **Run at M5:** `KEEL_MILESTONE=M5 cargo run -p conformance-runner -- --keelc target/release/keelc`
   — verify generics cases pass
