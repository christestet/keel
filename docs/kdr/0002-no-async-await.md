# KDR-0002: No async/await; runtime-scheduled structured concurrency only

- **Status:** accepted
- **Scope:** language

## Decision
Keel has no `async`/`await` and no function coloring. The runtime schedules
blocking operations (Go model). Concurrency is expressed only via structured
`scope { spawn ... }` blocks; tasks cannot outlive their scope; cancellation
propagates by scope exit.

## Context
Rust's async split the ecosystem (tokio vs others) and doubled its learning
curve; colored functions virally rewrite signatures (JS, Python, C#). Go proved
runtime scheduling serves backend workloads, but unstructured goroutines leak
(thousands of orphans, `context.Context` threading through every signature).

## Alternatives considered
Full async/await (rejected: coloring + ecosystem schism). Go-style free
goroutines (rejected: leaks, manual context plumbing). Effects systems
(rejected: research-grade learning curve, violates vision §10 positioning).

## Consequences
No detached background tasks — daemons are scopes owned by `main`. Some exotic
zero-overhead async patterns are unreachable. `context.Context` does not exist;
cancellation is ambient.

## Reopening clause
Corpus evidence of significant backend workload classes where the scheduler is
the measured bottleneck AND arenas/FFI cannot serve them.
