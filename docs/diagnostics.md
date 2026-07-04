# Compiler diagnostics

Keel diagnostics have stable `K####` codes. Tooling and conformance tests match
codes, not message text, so wording and help can improve without breaking the
diagnostic API.

The append-only source of truth is
[`compiler/keelc-diag/src/registry.rs`](../compiler/keelc-diag/src/registry.rs).
This page mirrors that registry for search and orientation. Registration does
not imply that a code is active in every milestone.

## Reading a diagnostic

```text
error[K0303]: cannot assign to immutable binding `x`
  --> main.keel:3:5
```

- `error` or `warning` is the severity;
- `K0303` is the stable machine-readable identity;
- the message describes this occurrence;
- the arrow identifies the primary source span.

Fix the source named by the code and span. Do not depend on exact message text
in scripts; use the process exit status and code.

## Code families

| Range | Area | Typical correction |
|---|---|---|
| `K00xx` | lexical and general syntax | correct the token, delimiter, string, or interpolation |
| `K01xx` | source conventions | use required casing and newline termination |
| `K02xx` | primitive type and arithmetic rules | make conversions/overflow behavior explicit |
| `K03xx` | declarations and bindings | complete declarations or use the correct mutability |
| `K04xx` | expressions and matching | align branch types and cover variants |
| `K05xx` | error propagation | align `Result`/`Option` contexts and handle error variants |
| `K06xx` | interfaces and impls | make the interface and implementation signatures identical |
| `K07xx` | structured concurrency | keep tasks inside their scope and observe results after joining |
| `K08xx` | constrained generics | add valid interface bounds and satisfying type arguments |
| `K09xx` | excluded/gated syntax | remove the feature or compile at the milestone where it exists |
| `K10xx` | memory regions | keep region-backed values inside their arena/scope |
| `K11xx` | packages and capabilities | correct `keel.toml`, dependencies, or declared authority |
| `K14xx` | editions and previews | select a recognized edition or required feature gate |
| `K15xx` | standard library | satisfy the module's static/runtime contract |
| `K16xx` | schema generation | correct the schema or remove unsupported constructs |

## Registered codes

Generated from
[`compiler/keelc-diag/src/registry.rs`](../compiler/keelc-diag/src/registry.rs)
by [`scripts/gen-diagnostics-doc.rs`](../scripts/gen-diagnostics-doc.rs). Do not
hand-edit the rows between the markers — register the code in the registry,
then run `scripts/gen-diagnostics-doc.rs --write`.
[`scripts/check-diagnostics-doc.sh`](../scripts/check-diagnostics-doc.sh) (part
of preflight and CI) fails the build if this table drifts from the registry.

<!-- gen:diagnostics:start -->
| Code | Registry summary |
|---|---|
| `K0001` | unrecognized character |
| `K0002` | unterminated string literal |
| `K0003` | syntax error |
| `K0004` | malformed string interpolation |
| `K0101` | identifier casing violation |
| `K0102` | semicolon used as a statement terminator |
| `K0201` | nullish construct used |
| `K0202` | implicit numeric conversion |
| `K0203` | integer overflow rule violation |
| `K0204` | division or remainder by zero |
| `K0301` | struct construction missing required field |
| `K0302` | function signature type annotation required |
| `K0303` | assignment to immutable binding |
| `K0401` | if/else arm type mismatch |
| `K0402` | non-exhaustive match |
| `K0403` | same-module enum wildcard match |
| `K0501` | ? used in incompatible return context |
| `K0502` | catch is not exhaustive |
| `K0503` | union error match is not exhaustive |
| `K0504` | cannot destructure opaque Error |
| `K0601` | interface declares more than five methods |
| `K0602` | duplicate method name in interface |
| `K0603` | missing method in impl |
| `K0604` | method signature mismatch in impl |
| `K0605` | type does not implement interface |
| `K0606` | method not found in interface |
| `K0607` | extraneous method in impl |
| `K0701` | spawn outside a scope |
| `K0702` | task result read before join barrier |
| `K0703` | task handle escapes its scope |
| `K0801` | type parameter without interface bound |
| `K0802` | method not in interface bound of type parameter |
| `K0803` | type argument does not satisfy interface bound |
| `K0804` | duplicate type parameter name |
| `K0805` | type parameter name shadows existing type |
| `K0806` | too many type parameters |
| `K0807` | interface used as generic constraint declares more than five methods |
| `K0901` | user-defined generics are not in Core |
| `K0902` | interfaces are not in Core |
| `K0903` | scope/spawn are not in Core |
| `K0904` | arena is not in Core |
| `K0905` | extern/FFI is not in Core |
| `K0906` | attributes are not in Core |
| `K0907` | operator overloading is not in Core |
| `K0908` | async/await are not in Core |
| `K1001` | arena reference escapes its block |
| `K1101` | manifest required but absent |
| `K1102` | malformed manifest |
| `K1103` | missing or invalid required manifest field |
| `K1104` | unknown manifest key |
| `K1105` | undeclared dependency in use path |
| `K1106` | unresolved dependency path |
| `K1107` | dependency cycle |
| `K1108` | package name collision |
| `K1110` | undeclared capability used |
| `K1111` | unknown capability name |
| `K1112` | dependency requires undeclared capability |
| `K1401` | unknown edition |
| `K1402` | preview feature used outside a preview build |
| `K1403` | idiom removed in the active edition |
| `K1501` | negative duration |
| `K1502` | invalid deadline type |
| `K1503` | unsupported JSON target |
| `K1504` | invalid HTTP handler |
| `K1505` | invalid HTTP port |
| `K1506` | invalid FromRow function |
| `K1507` | unparseable config target |
| `K1601` | malformed schema in keel gen |
| `K1602` | unsupported schema construct in keel gen |
<!-- gen:diagnostics:end -->

`K1402` and `K1403` are reserved but intentionally untriggered: Keel has no
approved preview feature or post-edition-1 removed idiom yet. The `K09xx`
not-in-Core cases are active only through the milestone where each feature
lands; at M7, generics, interfaces, structured concurrency, and arenas are
implemented while FFI remains rejected with `K0905`.

## Reporting a diagnostic defect

A panic, missing source span, nondeterministic ordering, or wrong stable code is
a compiler defect. Include the smallest source file that reproduces it, the
command including `--milestone`, and complete stderr. Do not change a
conformance expectation to accommodate the compiler.
