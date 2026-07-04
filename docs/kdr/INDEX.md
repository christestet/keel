# KDR Index

Derived from the decisions in [`docs/vision.md`](../vision.md). Each KDR
contains the decision, rationale, alternatives, and a reopening clause.

| # | Title | Status |
|---|---|---|
| [0000](0000-template.md) | KDR template | — |
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
| [0022](0022-interface-constrained-generics.md) | Interface-constrained generics | accepted |
| [0023](0023-impls-on-primitive-types.md) | User `impl` blocks on primitive types | accepted |
| [0024](0024-ai-infrastructure-and-agent-positioning.md) | AI-infrastructure and agent positioning | proposed |
| [0025](0025-structured-generation.md) | Type-driven structured generation | proposed |
| [0026](0026-structured-concurrency-mechanism.md) | Structured concurrency mechanism (spawn results, fail-fast, deadlines) | accepted |
| [0027](0027-json-boundary-mapping.md) | Typed JSON boundary mapping | accepted |
| [0028](0028-http-server-surface.md) | Typed HTTP server surface (`std.http`) | superseded-by-0031 |
| [0029](0029-sql-database-access.md) | Database access surface (`std.sql`) | accepted |
| [0030](0030-config-loading-surface.md) | Configuration loading surface (`std.config`) | accepted |
| [0031](0031-http-router-and-params.md) | HTTP Router and parameter extraction (`std.http`) | accepted |
| [0032](0032-call-site-type-args.md) | Type parameters and arguments use `<T>` | accepted |
| [0033](0033-universal-error-type.md) | Universal `Error` type — opaque boundary error sink | accepted |
| [0034](0034-core-boundary-scalars.md) | Core boundary scalars (`Uuid`, `Timestamp`, `Email`) | accepted |
| [0035](0035-multiline-string-literals.md) | Multi-line string literals (newlines inside `"..."`) | accepted |
| [0036](0036-default-function-parameters.md) | Default function parameters (`limit: Int = 50`) | accepted |
| [0037](0037-sql-error-classification-patterns.md) | `sql.Error` classification patterns + catch propagation | accepted |
| [0038](0038-union-narrowing-patterns.md) | Union narrowing patterns (typed bindings, `()`) | accepted |
| [0039](0039-option-unwrap.md) | `Option<T>.unwrap()` | accepted |
| [0040](0040-json-write-returns-string.md) | `json.write` returns `String` | accepted |
| [0041](0041-http-error-helpers-accept-error.md) | HTTP error response helpers accept `Error` | accepted |
| [0042](0042-sqlite-driver-modernc.md) | SQLite driver for the Go backend (`modernc.org/sqlite`) | accepted |
| [0043](0043-implicit-package-capability-trust-anchor.md) | Implicit packages are the capability trust anchor; `keel audit` reports derived capabilities | accepted |
| [0101](0101-compiler-in-rust.md) | Compiler implemented in Rust | accepted |
| [0102](0102-go-backend-first.md) | Go-emitting backend first, native before 1.0 | accepted |
| [0103](0103-lsp-server.md) | LSP server — protocol-driven editor integration | accepted |
| [0104](0104-keel-gen-codegen-surface.md) | `keel gen` — schema-driven codegen in the core toolchain | accepted |
| [0105](0105-hermetic-reproducible-builds.md) | Hermetic, reproducible builds | accepted |
| [0106](0106-query-engine.md) | Salsa query engine for keelc | accepted |
| [0107](0107-oci-image-build.md) | Daemonless, reproducible OCI image build (`keel build --image`) | accepted |
| [0108](0108-image-arch-selection.md) | `keel build --image --arch` target-architecture selection (amd64/arm64) | accepted |

All decisions derived from [`docs/vision.md`](../vision.md) are expanded above.
`proposed` entries are not yet accepted; see [`kdr/AGENTS.md`](AGENTS.md) for
expansion rules.
