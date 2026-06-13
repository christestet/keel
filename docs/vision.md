# Keel — Design Document v0.2

*A typed, compiled, garbage-collected language for backend services that should still be readable, reviewable, and deployable after five years of team churn.*

This revision resolves the open concerns from v0.1: ecosystem bootstrap, FFI safety, evolution governance, boundary serialization, compile-time budget, the GC escape hatch, rejection governance, linter waivers, stdlib funding, and the migration story. Where v0.1 stated philosophy, v0.2 states mechanism.

---

## 1. The Decision Record system (governance for everything below)

Every significant design choice in Keel — adopted or rejected — lives in a numbered, permanent **Keel Decision Record (KDR)**. A KDR contains the decision, the full rationale, the alternatives considered, and crucially a **reopening clause**: the specific, falsifiable evidence that would cause the decision to be revisited. "We reject macros" is not "macros are bad forever"; it is "macros are rejected, and this reopens only if the corpus shows ≥X% of real Keel code hand-writing the same boilerplate pattern that a corpus-tested macro design would eliminate."

This does three things at once. It ends relitigation (the answer to every "why doesn't Keel have X" thread is a link). It makes the project scientific rather than taste-driven, because reopening requires evidence, not advocacy. And it gives the community a legitimate path to change things, which is what prevents forks.

The evidence base is the **Keel Corpus**: a continuously updated, consent-based collection of open-source Keel code plus opt-in anonymized telemetry from the toolchain (build times, waiver counts, dependency graphs — never source). Every language RFC must include corpus analysis: how much real code does this change affect, simplify, or break? This is the Go team's proven internal practice (they ran every generics draft against Google's monorepo), made public and structural.

## 2. Ecosystem bootstrap: the niche, the multiplier, and the patron problem

Keel does not try to be a general-purpose ecosystem on day one. It picks one beachhead — **the containerized backend service** — and makes the day-one answer to "is there a library for X?" structurally different from other young languages, in three layers.

**Layer one: codegen as the ecosystem multiplier.** Most of what a backend service talks to is described by a machine-readable schema: protobuf, OpenAPI, SQL DDL, JSON Schema, AsyncAPI. Keel ships `keel gen` in the core toolchain: point it at a `.proto` file or an OpenAPI spec and it emits a fully typed client or server skeleton using only the stdlib. This means Keel "has" a typed client for Stripe, Kubernetes, or any internal gRPC service the moment it exists — without anyone maintaining a Stripe package. The first hundred integrations are generated, not written. This is the single highest-leverage bootstrap decision in the design.

**Layer two: the C FFI as a bridge, governed by capabilities (see §3).** For the things schemas can't describe — Kafka's wire protocol, librdkafka, sqlite, libpq edge cases — Keel wraps existing battle-tested C libraries early rather than rewriting them. Wrapping is explicitly a bridge strategy: each official wrapper carries a KDR-tracked plan stating whether it stays a wrapper forever or gets a pure-Keel replacement once usage justifies it.

**Layer three: `x.keel.dev`, the extended library.** A single official namespace, maintained with stdlib discipline (same review bar, same compatibility promise, same security process) but versioned independently of the compiler so it can move faster. Kafka, Redis, cloud SDKs, OIDC live here. Promotion path is explicit and corpus-driven: community package → adopted into `x` when usage crosses a threshold and maintainers accept the discipline → into `std` only via an edition (§5).

**The patron problem, answered honestly.** Languages survive on funding, not enthusiasm. Keel's governance is a foundation from day one (not a single-company project that later "donates" the language), with a funding treaty: every component in `std` and `x` must have a *named, paid* maintainer of record before it is admitted. A library nobody is paid to maintain does not enter the official namespaces — it stays community. This is the most expensive sentence in the design, and writing it down is the point: the bundled-Postgres-driver promise is a perpetual cost, and admitting components without funding them is how stdlibs rot (Python's `urllib`, Go's `encoding/xml`). Realistically this means Keel needs an anchor sponsor whose business runs on it — the honest prerequisite every successful language had (Go/Google, Rust/Mozilla-then-AWS+Microsoft, Kotlin/JetBrains).

## 3. Safety at the boundary: capabilities, not trust

v0.1 said FFI should be "loud." v0.2 generalizes this into Keel's most distinctive safety feature: **package capabilities**.

Every package manifest must declare what it touches: `net`, `fs`, `exec`, `env`, `ffi`, `unsafe-memory`. The compiler enforces the declaration — a package without `net` cannot reach the socket API, transitively. The build fails if your dependency tree's actual behavior exceeds its declared capabilities.

```toml
# keel.toml of a JSON-schema validation package
[capabilities]
# (empty — this package computes. It cannot phone home, read your
#  filesystem, or exfiltrate your env vars. The compiler guarantees it.)
```

`keel audit` then becomes a one-screen answer to the supply-chain question: *which of my 9 dependencies can open a network connection, and which cross the FFI boundary?* A left-pad-style utility package that requests `net` is visibly absurd before anyone reads its source. This addresses the npm-culture rejection structurally rather than by registry policy: micro-dependencies become low-risk by construction, and high-capability packages concentrate scrutiny where it matters.

FFI specifically: `extern` blocks are the only door, they require the `ffi` capability, every crossing appears in the audit report and the SBOM, and `extern` code is excluded from Keel's safety guarantees with a mandatory documented contract (what the C side may do with each pointer). Builds themselves are sandboxed and hermetic — no build scripts, no arbitrary code execution at compile time (KDR-0007), so `keel build` on untrusted code is safe by definition.

## 4. The memory model, completed: GC plus scoped arenas

The v0.1 question — "what's the sanctioned escape hatch when GC isn't enough?" — gets one answer, chosen because it rhymes with everything else in the language: **arenas are scopes**.

```keel
fn parse_huge_feed(input: Bytes) -> Summary {
    arena {
        let nodes = parse(input)        // all allocations land in the arena
        summarize(nodes)                // result is copied out at the boundary
    }                                   // arena freed in O(1); GC never saw it
}
```

An `arena` block is region allocation with the same shape as `scope` blocks in concurrency and resource cleanup: lexical, visible, impossible to leak, and checked — the compiler's escape analysis forbids arena references from outliving the block (this is the *same analysis* the GC already needs, pointed at a new rule, so it costs no new conceptual machinery). There is no manual `free`, no lifetimes annotation syntax, no second memory paradigm to learn. Hot paths — parsers, request-scoped object graphs, caches rebuilt per tick — get deterministic, GC-invisible allocation; everyone else never types the keyword. KDR-0012 records the rejected alternatives (ownership annotations, `sync.Pool`-style folklore, "just use Rust via FFI") and the reopening clause (evidence of significant real-world workloads that arenas + GC cannot serve).

The container-awareness promise from v0.1 stands and extends: the runtime reads cgroup limits, and `keel build` can emit a **runtime profile** (`--profile latency` vs `--profile throughput`) that tunes GC pacing — a build-time choice, not a 40-environment-variable tuning surface.

## 5. Evolution: editions with teeth, on a clock

Keel adopts Rust's editions model with two hardenings, recorded as KDR-0001 because everything else depends on it.

**Editions are exclusive.** When an edition replaces an idiom, the old idiom becomes a *compile error* in the new edition — not a deprecation warning that lives forever. The "one way to do things" promise is therefore scoped per edition and mechanically enforced. Code on old editions keeps compiling forever (the compiler supports all editions), so nobody is forced to migrate — but a codebase cannot mix eras within one module.

**Migration is a deliverable, not a suggestion.** No edition change ships unless `keel fix` migrates the entire public corpus automatically with zero semantic diffs. If the migration can't be written mechanically, the change is redesigned until it can. This inverts the usual dynamic: the burden of evolution falls on the language team, once, instead of on every user, repeatedly.

Editions arrive on a fixed three-year cadence (predictability beats perfection), and each edition is an LTS: security fixes for the toolchain and stdlib for the full overlap window. The RFC process feeding editions requires corpus evidence (§1) and ships experimental features only behind `keel build --preview=<feature>`, which refuses to run outside CI-marked builds — you can experiment, you cannot deploy a preview.

## 6. The boundary doctrine: parse, don't validate — ergonomically

Inside a Keel program, illegal states are unrepresentable. The network does not care. KDR-0015 fixes the doctrine for the edge:

External data enters only through explicit parse points (`json.parse<T>`, `proto.decode<T>`, `sql` row mapping), and the type `T` must honestly describe the wire reality: any field the schema does not guarantee is `Option<T>` — the compiler rejects a required field for optional wire data when a schema is available to check against. Parsing is **strict by default** (unknown fields are errors, catching typos and contract drift in dev) with an explicit, visible relaxation for the real world: `json.parse<T>(body, mode: .tolerant)` ignores unknown fields and logs a structured `schema_drift` event to OTel. Tolerance is a choice you can grep for and an event you can alert on — not a silent default (Go) or an all-or-nothing wall.

The ergonomic key is that you usually don't hand-write boundary types at all: `keel gen` (§2) derives them from the proto/OpenAPI source of truth, so the honest-`Option` rule costs nothing in typing and the "five-year-old API with misspelled fields" case is handled by a generated type plus one `.tolerant`.

## 7. Compile time as a contract

The build budget is a public, versioned artifact, not an aspiration (KDR-0019). The reference benchmark (a realistic 100k-LOC service suite, in the open) must satisfy: **cold build < 10s, incremental build < 1s, `keel check` (types + lint, no codegen) < 300ms** on the reference laptop. CI treats a regression beyond 5% as a release blocker, identical in severity to a miscompilation. This line exists because the failure mode is sneaky: SQL checking, exhaustiveness, capability verification, and structural-diff assertions are each cheap, and their sum is how a fast compiler becomes Rust's. Specific mechanics: compile-time SQL verification is cached against the migration set's hash (re-checked only when SQL or schema text changes), the compiler is architected around incrementality from day one rather than retrofitted (Rust's most expensive lesson), and `keel check` is the editor/CI fast path so humans rarely wait on full codegen.

## 8. Waivers: configurable in public only

The non-configurable linter gets exactly one pressure valve (KDR-0018), designed to be socially expensive instead of silently convenient:

```keel
// keel:waiver(rule: complexity, reason: "generated protocol state machine", issue: "PLAT-2241", expires: edition-2031)
```

A waiver requires a rule, a reason, a tracking issue, and an expiry (a date or an edition). Expired waivers are compile errors. Every waiver appears in `keel audit`, in the test summary, and as a count in the build output — the team sees `waivers: 3` next to `coverage: 94%` on every build. Nothing is hidden, nothing is permanent, and the global corpus statistics on waiver usage are exactly the evidence stream (§1) that tells the language team which lint rules are wrong in practice. Configurability through shame plus telemetry, not through a config file.

## 9. The migration story is a product

Keel's adoption unit is not "a company switches languages"; it is **one new microservice inside an existing Go/Java/Node estate, in one afternoon**. That story is a maintained, CI-tested deliverable called the **landing kit**: `keel init service --from proto ./order.proto` scaffolds a service that speaks the org's existing gRPC contracts (via §2 codegen), emits OTel traces/metrics/logs in standard semantic conventions so existing dashboards light up unchanged, answers the platform team's standard probes (`/healthz`, `/readyz`, SIGTERM drain), ships the two-line `FROM scratch` Dockerfile plus a reference Helm chart, and produces an SBOM their security tooling already ingests. Interop with the incumbent world is not a feature checklist; it is the funnel, and it gets an owner and CI like any product. The full bootstrap strategy is recorded as KDR-0020.

## 10. Positioning, stated so it can't drift (KDR-0021)

A permanent "Who Keel is not for" document ships next to the tutorial. Keel is wrong for game engines, kernels, embedded targets, GUI apps, scientific computing, and any domain needing deterministic sub-100µs latency or manual memory control — use Rust, C, or Zig, and Keel's FFI will happily call the result. Keel will bore some excellent engineers, and that is the design working: the optimization target is *the team across five years*, not the individual across five hours. Expressiveness requests that conflict with this are answered by KDR link, not by negotiation. Honesty here is strategic: every language that promised universality diluted itself; every language that named its lane (SQL, Erlang, Go-as-it-began) is still in it.

---

## Appendix — Canonical reference documents

| Topic | Location |
|---|---|
| Decision records | [`docs/kdr/INDEX.md`](kdr/INDEX.md) — all KDRs; those derived from this document are marked in each KDR's Context section |
| Build order / milestone sequence | [`ROADMAP.md`](../ROADMAP.md) — exit criteria and ordering constraints |
| Implementation status | [`docs/milestone-status.md`](milestone-status.md) — current build-out per milestone |
