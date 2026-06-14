# Specification plan

The full normative spec grows chapter by chapter, each landing together with its
conformance tests (spec PR → test PR → implementation PR, per AGENTS.md).

Planned chapters:

| # | Topic | Governing KDR(s) |
|---|-------|------------------|
| 01 | Lexical structure | [KDR-0014](kdr/0014-interpolation-brace-escaping.md) |
| 02 | Types | — |
| 03 | Declarations | [KDR-0003](kdr/0003-no-inheritance.md) |
| 04 | Expressions | [KDR-0013](kdr/0013-core-operators-and-integer-division.md), [KDR-0009](kdr/0009-no-operator-overloading.md) |
| 05 | Errors | [KDR-0005](kdr/0005-no-exceptions.md) |
| 06 | Modules / packages | [KDR-0011](kdr/0011-package-capabilities.md), [KDR-0017](kdr/0017-function-capabilities.md) |
| 07 | Interfaces | [KDR-0003](kdr/0003-no-inheritance.md) — **landed**, see [`07-interfaces.md`](07-interfaces.md) and [`docs/milestone-status.md`](../milestone-status.md) §M5 |
| 08 | Generics | [KDR-0022](../kdr/0022-interface-constrained-generics.md) — **landed**, see [`08-generics.md`](08-generics.md) and [`docs/generics-implementation.md`](../generics-implementation.md) |
| 09 | Concurrency (scope/spawn) | [KDR-0002](kdr/0002-no-async-await.md) |
| 10 | Memory (GC + arena) | [KDR-0012](kdr/0012-gc-plus-scoped-arenas.md), [KDR-0016](kdr/0016-scope-implicit-arenas.md) |
| 11 | Capabilities | [KDR-0011](kdr/0011-package-capabilities.md), [KDR-0017](kdr/0017-function-capabilities.md) |
| 12 | FFI | [KDR-0011](kdr/0011-package-capabilities.md) |
| 13 | Testing | — |
| 14 | Editions | [KDR-0001](kdr/0001-editions.md) |
| 15 | Stdlib core | — |
| 16 | LSP server protocol | [KDR-0103](../kdr/0103-lsp-server.md) — **landed**, see [`16-lsp.md`](16-lsp.md) |

Until a chapter exists, `keel-core.md` plus the conformance suite is the only
normative text. Style: every normative statement is testable; every error gets a
stable K#### code; examples in spec chapters are extracted and run by CI
(literate-spec discipline, like the Rust reference's tested examples).
