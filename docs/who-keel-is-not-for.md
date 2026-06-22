# Who Keel is not for

Keel is a language for typed backend services maintained by changing teams over
years. It is deliberately not a general-purpose answer to every software
domain. This boundary is governed by accepted
[KDR-0021](kdr/0021-positioning.md).

## Use another language for

### Kernels, drivers, and embedded systems

Keel has garbage collection, no manual memory management, and no ownership or
lifetime syntax. It does not expose the deterministic allocation, layout, or
hardware control required by kernels and constrained embedded targets.

Use C, Rust, Zig, or the platform's established systems language.

### Hard real-time and deterministic sub-100µs latency

Keel's runtime model includes a concurrent garbage collector and scheduler.
Scoped arenas reduce GC pressure but do not turn the language into a hard
real-time environment.

Use a language/runtime with a demonstrated worst-case latency contract.

### Game engines

Keel does not target frame-oriented memory control, data-oriented engine
layouts, graphics APIs, or engine/editor ecosystems. Arena syntax is not a
replacement for a game-engine allocation model.

Use C++, C#, Rust, or the language supported by the chosen engine.

### Desktop and mobile GUI applications

Keel has no GUI toolkit, rendering model, application lifecycle, or native UI
interop story. Adding those would dilute the backend-service standard library
and tooling priorities.

Use the platform's native stack or an established cross-platform UI framework.

### Scientific and numerical computing

Keel has no array-programming model, accelerator support, numerical package
ecosystem, or floating-point reproducibility contract suitable for scientific
workloads.

Use Python with its numerical ecosystem, Julia, R, Fortran, C++, or a
domain-specific tool.

### Software requiring manual memory control

Keel intentionally provides GC plus scoped arenas, not `malloc`/`free`, pointer
arithmetic, ownership types, or user-visible lifetimes. If manual control is a
requirement rather than a measured exception, Keel is the wrong language.

## What Keel does target

Keel is designed for services that:

- expose HTTP or schema-described APIs;
- use JSON, SQL, environment configuration, and structured concurrency;
- benefit from exhaustive types and explicit error propagation;
- need auditable dependency capabilities and reproducible builds;
- are maintained by teams whose membership changes over time.

The optimization target is the team across five years, not maximum expression
density for one author today.

## Interoperability is not universality

The design includes a future C FFI so a Keel service can call focused systems
components. That does not make Keel suitable for implementing those components.
The FFI is planned for M10 and is not currently implemented.

## How this constrains proposals

A feature proposal must explain how it improves Keel's backend-service niche
without weakening readability, deterministic tooling, capability auditing, or
the one-way-to-write-it goal. Demand from an excluded domain is not sufficient
evidence for expanding the language.

KDR-0021 says the positioning can reopen if production corpus evidence shows
that the niche is unsustainably narrow or routinely ignored. It also requires a
quantitative threshold to live in this document, but no accepted decision has
chosen that number. The threshold must be settled through the KDR process before
it becomes normative; this document does not invent one.
