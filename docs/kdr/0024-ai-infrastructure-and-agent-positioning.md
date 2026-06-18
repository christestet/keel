# KDR-0024: AI-infrastructure and agent positioning

- **Status:** proposed
- **Date:** 2026-06-18
- **Scope:** governance

## Decision

Keel adopts **AI infrastructure and agent/genAI systems as an explicit
first-class vertical inside its existing backend-service lane** (consistent with
[`KDR-0021`](0021-positioning.md); AI serving is a backend service, not
scientific computing). Keel is the language and toolchain for **model-serving
APIs, inference gateways, retrieval/RAG and feature pipelines, agent
orchestration, evaluation harnesses, and MLOps plumbing**. All numerical and
training computation — tensors, autodiff, accelerator kernels — is **delegated
across the capability-governed FFI** to existing systems (PyTorch, JAX, Triton,
cuDNN, BLAS, llama.cpp). Keel does **not** become a numerical, differentiable, or
GPU-kernel language; that path stays closed (it would collide with
[`KDR-0009`](0009-no-operator-overloading.md), [`KDR-0004`](0004-no-macros.md),
[`KDR-0012`](0012-gc-plus-scoped-arenas.md), and
[`KDR-0022`](0022-interface-constrained-generics.md), and lose to ecosystem
gravity).

Three **compiler-level differentiators** define the vertical and justify Keel
over "Python + a web framework":

1. **Capability-scoped tools** — an agent-invokable tool is a typed function
   whose side-effecting authority (`net`, `fs`, `exec`, …) is *declared and
   compiler-enforced* ([`KDR-0011`](0011-package-capabilities.md),
   [`KDR-0017`](0017-function-capabilities.md)). An agent may wire in only tools
   whose capabilities fall within its granted budget, giving **provable
   prompt-injection / tool-misuse containment** — statically, not via a runtime
   sandbox.
2. **Type-driven structured generation** — LLM output is parsed into Keel types
   through compiler-derived schemas/grammars, extending the boundary doctrine
   ([`KDR-0015`](0015-boundary-doctrine.md)) to model outputs. Specified
   separately in [`KDR-0025`](0025-structured-generation.md).
3. **Streaming + structured concurrency** — token streams as a first-class type,
   and bounded, cancelable, deadline-propagating agent orchestration built on
   the M5 `scope`/`spawn` model.

Everything else — model clients, tokenizers, embeddings, vector stores, tracing,
retries — is **library/codegen** (`std`, `x.keel.dev`, `keel gen`), not language
surface.

## Context

The 2023–2026 shift in AI moved the hard, high-value problems from "train a model
from scratch" to **serving, fine-tuning, and orchestrating large pretrained
models**: inference gateways, quantization, KV-cache and parallelism, structured
generation, and agent loops (vLLM, TensorRT-LLM, SGLang, llama.cpp). That is
**systems and infrastructure work**, which is exactly Keel's lane.

The recurring pains in the incumbent stack are, point for point, things Keel's
*accepted* decisions already address:

| Incumbent pain | Keel decision that addresses it |
|---|---|
| Packaging / CUDA-matrix / "works on my machine" | Hermetic, sandboxed builds + static binaries + SBOM ([`KDR-0007`](0007-no-build-scripts.md)) |
| Agents executing tools = prompt-injection RCE | Compiler-enforced capabilities ([`KDR-0011`](0011-package-capabilities.md)/[`KDR-0017`](0017-function-capabilities.md)) |
| LLM output is untrusted, schema-drifting data | Parse-don't-validate, honest `Option`, strict+`.tolerant` ([`KDR-0015`](0015-boundary-doctrine.md)) |
| Reproducibility crisis (seeds, dep drift, nondeterminism) | Determinism + hermetic builds; record/replay harness |
| Runaway agent loops, leaked tasks/timeouts | Structured concurrency (`scope`/`spawn`, M5) |
| Flaky model/tool calls | `Result` + `?` + `catch`, uncatchable panics ([`KDR-0005`](0005-no-exceptions.md)) |
| Integration boilerplate | `keel gen` from OpenAPI/proto/schema ([`KDR-0020`](0020-ecosystem-bootstrap.md), vision §2) |

Prior-art lessons that shape the boundary of this decision:

- **Swift for TensorFlow (dead 2021)** — superb compiler-integrated autodiff,
  killed by ecosystem gravity. *Do not fight Python+CUDA head-on on numerics.*
- **Julia** — solved the two-language problem technically, never overcame
  Python's gravity; latency/composability footguns hurt adoption. *Technical
  superiority is not adoption.*
- **Mojo** — confirms the demand (a company bet on it) and that MLIR is the
  numeric substrate; also confirms "be a numerics language" is a multi-year,
  capital-intensive war.
- **Rust** — won ML *infrastructure* (Polars, tokenizers, candle/inference) while
  staying painful for research. *A typed systems language wins the infra/serving
  lane, not the notebook lane* — the template for Keel here.

## Alternatives considered

- **Become a numerical / training / autodiff language** (Julia/Mojo path).
  Rejected: collides with four accepted KDRs (0009, 0004, 0012, 0022) and loses
  to ecosystem gravity (S4TF). The numeric world is reachable, and better
  served, through capability-governed FFI.
- **Stay a generic backend language; do not name an AI lane.** Rejected: Keel's
  accepted design reads almost like a spec for an agent-infra language;
  declining to name the vertical cedes a near-perfect fit, contrary to
  [`KDR-0021`](0021-positioning.md)'s "name your lane" discipline.
- **Library-only AI story (clients + helpers, no compiler features).** Rejected:
  the two differentiators that constitute a moat — capability-scoped tools and
  type-driven structured generation — require compiler integration. A
  library-only story is "Python + FastAPI with extra steps" and defensible by no
  one.

## Consequences

- **Authorizes dependent work** (each its own KDR/PR when scheduled): accept and
  implement [`KDR-0017`](0017-function-capabilities.md); the structured-generation
  mechanism [`KDR-0025`](0025-structured-generation.md); a future "capability-scoped
  tool" language KDR; an `x.keel.dev/ai` namespace (typed model clients,
  tokenizers, vector stores) governed by the `x` funding/discipline bar
  ([`KDR-0020`](0020-ecosystem-bootstrap.md)).
- **Observability**: extend the landing-kit OTel story to the GenAI semantic
  conventions (model, tokens, cost, tool spans).
- **Milestone placement**: post-M6 — it depends on the stdlib slice (`std.http`,
  `std.json`) and on M5 structured concurrency landing first. Nothing here
  relaxes a current milestone exit criterion.
- **Excluded, on purpose**: numeric kernels, autodiff, GPU codegen, runtime
  reflection-based schema magic. These remain closed and are reached via FFI.
- **Positioning doc** ([`KDR-0021`](0021-positioning.md)) gains a clarifying line:
  Keel is for AI *infrastructure/serving/agents*, not AI *training/numerics*.
- Inconveniences researchers wanting to prototype models in Keel; that audience
  is explicitly redirected to Python/Julia/Mojo + FFI.

### Non-normative syntax sketch (illustrative only — not a decided surface)

```keel
// A tool: a typed function the agent runtime may invoke. Its capability
// set is declared and compiler-enforced (KDR-0011/0017); the schema the
// model sees is derived from the signature (KDR-0025).
tool search_web(query: String) -> List<SearchResult> uses net {
    // only `net` is permitted in here, transitively
}

tool read_note(id: NoteId) -> Result<Note, NotFound> uses fs.read {
    // opening a socket here is a compile error
}

fn run_assistant(input: String) -> Result<String, AgentError> uses net {
    let agent = Agent {
        model: anthropic.claude(),
        tools: [search_web],   // read_note rejected: `fs` exceeds this budget
    }
    agent.run(input)?
}

// Type-driven structured generation (KDR-0025): output parsed into T,
// schema derived by the compiler, strict-by-default boundary (KDR-0015).
struct Triage { severity: Severity, summary: String, owner: Option<String> }

fn triage(ticket: String) -> Result<Triage, ModelError> uses net {
    generate<Triage>(model: anthropic.claude(), prompt: "Triage: {ticket}")
}
```

## Reopening clause  *(required)*

Reopen if corpus evidence from at least three distinct Keel AI-infrastructure
codebases each exceeding 10,000 lines shows **either**:

1. that the capability-scoped-tool + structured-generation model fails to contain
   a demonstrated tool-safety / prompt-injection bug class that a conventional
   runtime sandbox *would* have caught — i.e. the static model provides no real
   safety advantage; **or**
2. that the "infra-not-numerics, delegate via FFI" split imposes a measured
   productivity or performance regression beyond an agreed bound (fixed in the
   reopening proposal, e.g. a named latency/throughput or lines-of-code metric)
   with no workaround via FFI, `keel gen`, or `x.keel.dev` — making in-language
   numerics necessary.

Market hype, model-of-the-month excitement, and "language X is doing AI" are
never sufficient.
