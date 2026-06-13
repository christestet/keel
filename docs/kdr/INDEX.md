# KDR Index

Derived from the decisions in [`docs/vision.md`](../vision.md). Each KDR
contains the decision, rationale, alternatives, and a reopening clause.

| # | Title | Status |
|---|---|---|
| [0001](0001-editions.md) | Exclusive editions, mandatory mechanical migration | accepted |
| [0002](0002-no-async-await.md) | No async/await; structured concurrency only | accepted |
| 0003 | No inheritance (composition + interfaces ≤5 methods) | accepted — stub |
| [0004](0004-no-macros.md) | No macros / metaprogramming / reflection | accepted |
| 0005 | No exceptions; Result + ? + catch; uncatchable panics | accepted — stub |
| 0006 | No conditional compilation beyond OS/arch | accepted — stub |
| 0007 | No build scripts; hermetic sandboxed builds | accepted — stub |
| 0008 | No reflection (folded into [0004](0004-no-macros.md)) | superseded |
| 0009 | No operator overloading / implicit conversions | accepted — stub |
| 0010 | One formatter, zero options, compile-enforced | accepted — stub |
| 0011 | Package capabilities (net/fs/exec/ffi) | accepted — stub |
| 0012 | GC + scoped arenas; no ownership/lifetimes | accepted — stub |
| [0013](0013-core-operators-and-integer-division.md) | Core operator set and integer division semantics | accepted |
| [0014](0014-interpolation-brace-escaping.md) | Brace escaping in string interpolation | accepted |
| 0015 | Boundary doctrine: parse don't validate, strict default | accepted — stub |
| [0101](0101-compiler-in-rust.md) | Compiler implemented in Rust | accepted |
| [0102](0102-go-backend-first.md) | Go-emitting backend first, native before 1.0 | accepted |
| [0000](0000-template.md) | KDR template | — |

**Stubs** are accepted decisions that have not yet been expanded from vision.md
into a full KDR file. See [`kdr/AGENTS.md`](AGENTS.md) for expansion rules.
