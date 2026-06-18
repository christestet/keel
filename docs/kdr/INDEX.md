# KDR Index

Derived from the decisions in [`docs/vision.md`](../vision.md). Each KDR
contains the decision, rationale, alternatives, and a reopening clause.

| # | Title | Status |
|---|---|---|
| 0000 | KDR template | — |
| [0001](0001-editions.md) | Exclusive editions, mandatory mechanical migration | accepted |
| [0002](0002-no-async-await.md) | No async/await; structured concurrency only | accepted |
| [0003](0003-no-inheritance.md) | No inheritance (composition + interfaces ≤5 methods) | accepted |
| [0004](0004-no-macros.md) | No macros / metaprogramming / reflection | accepted |
| [0005](0005-no-exceptions.md) | No exceptions; Result + ? + catch; uncatchable panics | accepted |
| [0006](0006-no-conditional-compilation.md) | No conditional compilation beyond OS/arch | accepted |
| [0007](0007-no-build-scripts.md) | No build scripts; hermetic sandboxed builds | accepted |
| 0008 | No reflection (folded into [0004](0004-no-macros.md)) | superseded |
| [0009](0009-no-operator-overloading.md) | No operator overloading / implicit conversions | accepted |
| [0010](0010-one-formatter.md) | One formatter, zero options, compile-enforced | accepted |
| [0011](0011-package-capabilities.md) | Package capabilities (net/fs/exec/ffi) | accepted |
| [0012](0012-gc-plus-scoped-arenas.md) | GC + scoped arenas; no ownership/lifetimes | accepted |
| [0013](0013-core-operators-and-integer-division.md) | Core operator set and integer division semantics | accepted |
| [0014](0014-interpolation-brace-escaping.md) | Brace escaping in string interpolation | accepted |
| [0015](0015-boundary-doctrine.md) | Boundary doctrine: parse don't validate, strict default | accepted |
| [0016](0016-scope-implicit-arenas.md) | Scope-implicit arenas via structured concurrency | proposed |
| [0017](0017-function-capabilities.md) | Function-level capability annotations | proposed |
| [0018](0018-waivers.md) | Waivers — configurable in public only | accepted |
| [0019](0019-compile-time-contract.md) | Compile time as a contract | accepted |
| [0020](0020-ecosystem-bootstrap.md) | Ecosystem bootstrap strategy | accepted |
| [0021](0021-positioning.md) | Positioning and scope discipline | accepted |
| [0022](0022-interface-constrained-generics.md) | Interface-constrained generics | proposed |
| [0023](0023-impls-on-primitive-types.md) | User `impl` blocks on primitive types | proposed |
| [0101](0101-compiler-in-rust.md) | Compiler implemented in Rust | accepted |
| [0102](0102-go-backend-first.md) | Go-emitting backend first, native before 1.0 | accepted |
| [0103](0103-lsp-server.md) | LSP server — protocol-driven editor integration | proposed |

All decisions derived from [`docs/vision.md`](../vision.md) are expanded above.
`proposed` entries are not yet accepted; see [`kdr/AGENTS.md`](AGENTS.md) for
expansion rules.
