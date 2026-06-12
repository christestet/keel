# KDR Index

| # | Title | Status |
|---|---|---|
| 0001 | Exclusive editions, mandatory mechanical migration | accepted |
| 0002 | No async/await; structured concurrency only | accepted |
| 0003 | No inheritance (composition + interfaces ≤5 methods) | accepted — stub, expand from vision.md |
| 0004 | No macros / metaprogramming / reflection | accepted |
| 0005 | No exceptions; Result + ? + catch; uncatchable panics | accepted — stub |
| 0006 | No conditional compilation beyond OS/arch | accepted — stub |
| 0007 | No build scripts; hermetic sandboxed builds | accepted — stub |
| 0008 | No reflection (folded into 0004) | accepted |
| 0009 | No operator overloading / implicit conversions | accepted — stub |
| 0010 | One formatter, zero options, compile-enforced | accepted — stub |
| 0011 | Package capabilities (net/fs/exec/ffi) | accepted — stub, vision.md §3 |
| 0012 | GC + scoped arenas; no ownership/lifetimes | accepted — stub, vision.md §4 |
| 0013 | Core operator set and integer division semantics | accepted |
| 0014 | Brace escaping in string interpolation | accepted |
| 0015 | Boundary doctrine: parse don't validate, strict default | accepted — stub, vision.md §6 |
| 0101 | Compiler implemented in Rust | accepted |
| 0102 | Go-emitting backend first, native before 1.0 | accepted |

Stubs: copy the relevant vision.md section into the template. Good first
contribution for a new (human or LLM) contributor — no design work, only
faithful transcription plus a reopening clause for review.
