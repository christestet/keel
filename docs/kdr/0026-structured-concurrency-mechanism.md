# KDR-0026: Structured concurrency mechanism — spawn results, fail-fast, deadline propagation

- **Status:** proposed
- **Date:** 2026-06-18
- **Scope:** language

## Decision

This KDR fixes the operational mechanism of the `scope`/`spawn` model that
[`KDR-0002`](0002-no-async-await.md) decided in shape but left abstract. It does
**not** reopen KDR-0002: no async/await, no function coloring, runtime
scheduling, tasks cannot outlive their scope, cancellation propagates by scope
exit. It pins *how*.

1. **Join barrier.** `scope { ... }` delimits a task tree. The closing brace is
   an implicit join barrier: control does not pass it until every task spawned in
   the scope has terminated (completed, failed, or finished unwinding a
   cancellation). This is KDR-0002's "tasks cannot outlive their scope" made
   operational — there is no way to express a detached task.

2. **Spawn handles.** `spawn e` launches a child task evaluating `e` and is itself
   an expression of type `Task<T>` (where `e: T`). A handle's result is observable
   **only after** the join barrier; reading a handle's value inside the scope body
   before join is a compile error. The scope block is an expression whose value is
   the collection of its tasks' results (exact collection surface — list, tuple,
   or named handles — is fixed in spec chapter 09).

3. **Fail-fast by default.** If any child task fails (returns `Err`, per
   [`KDR-0005`](0005-no-exceptions.md)), the scope immediately requests
   cancellation of all sibling tasks, waits for them to unwind, and the `scope`
   expression evaluates to that failure. The scope therefore composes with `?`:
   `let rows = scope { ... }?`. When several tasks have failed by the time the
   barrier resolves, the reported error is selected **deterministically by spawn
   order** (lowest-indexed failing task wins), satisfying the determinism rule
   (root `AGENTS.md` hard rule 7); scheduling order itself stays unobservable. A
   non-fail-fast "collect every outcome" mode is named here but deferred to a
   future KDR — the default is opinionated on purpose.

4. **Deadlines are ambient and monotone.** A scope may carry a deadline. The
   deadline propagates implicitly to every descendant task and nested scope — it
   is **never threaded through function signatures** (KDR-0002: "`context.Context`
   does not exist; cancellation is ambient"). A nested scope may only *tighten*
   an inherited deadline, never extend it. On expiry the runtime triggers the same
   cooperative-cancellation path as fail-fast; the scope evaluates to a
   `Cancelled` error value — recoverable via `catch`, **not** a panic.

5. **Cancellation is cooperative and value-based, never a panic.** Cancellation is
   delivered at checkpoints: every runtime blocking operation (I/O, channel ops,
   sleep, a nested join) is a checkpoint, plus an explicit yield (`check_cancel()`)
   for compute-bound loops. A cancelled operation returns `Cancelled` through the
   ordinary `Result` channel, so cleanup runs through the normal scope-exit /
   `catch` path. Panics (KDR-0005) remain uncatchable and orthogonal: a panicking
   task aborts the process; it does not become a recoverable cancellation.

Spawned tasks run under the enclosing function's capability budget
([`KDR-0017`](0017-function-capabilities.md)); `spawn` grants no new authority.
The scope's allocation region ([`KDR-0016`](0016-scope-implicit-arenas.md)) is
freed at the join barrier — sound precisely because join guarantees no child
outlives the scope.

### Non-normative syntax sketch (illustrative only — surface decided in chapter 09)

```keel
fn fan_out(ids: List<Id>) -> Result<List<Row>, DbError> {
    scope(deadline: 2.seconds) {
        // each spawn is a Task<Row>; the scope collects them in spawn order.
        // if one db.get fails, siblings are cancelled and the first error
        // (by spawn index) propagates out through `?` below.
        ids.map(|id| spawn db.get(id))
    }?                              // join barrier; List<Row> on success
}

fn poll_loop(input: Stream) uses net {
    scope {
        spawn forever {
            check_cancel()          // cooperative checkpoint for a compute loop
            handle(input.next())
        }
    }                               // scope owned by caller; no detached task
}
```

## Context

KDR-0002 chose structured concurrency over async/await and over free goroutines,
but a "scope" with unspecified failure and result semantics is not implementable
or testable. The concrete questions every structured-concurrency system must
answer — *what does a spawned task return, what happens when one of several
siblings fails, and how do timeouts reach a task three levels deep* — are exactly
what KDR-0002 left open and what spec chapter 09 cannot be written without.

Prior art the mechanism draws on:

- **Trio's nurseries / Python `TaskGroup`** — the join-barrier + fail-fast model;
  proved that "the block doesn't exit until its children do" eliminates the orphan
  class KDR-0002 targets. Adopted.
- **Go `errgroup` + `context`** — first-error-cancels-the-group is the de-facto
  backend pattern; Keel adopts the semantics but makes the context *ambient*
  rather than a parameter threaded through every signature (the cost Go pays).
- **Kotlin structured concurrency / Swift `withTaskGroup`** — confirm deadline and
  cancellation must inherit down the task tree automatically; explicit per-call
  timeout plumbing is the failure mode to avoid.
- **Java Project Loom `StructuredTaskScope`** — confirms cooperative cancellation
  at blocking points is sufficient for I/O-bound backend work without preemption.

Backend feasibility (Go, [`KDR-0102`](0102-go-backend-first.md)): `scope` lowers
to an `errgroup.Group` over a derived `context.Context`; `spawn` to `g.Go`; the
closing brace to `g.Wait`; `scope(deadline:)` to `context.WithDeadline`;
checkpoints poll `ctx.Err()`. The user never sees the context — it is the ambient
machinery KDR-0002 specified.

## Alternatives considered

- **Spawn returns nothing; results only via shared mutable state.** Rejected:
  forces channel/mutex plumbing for the common "fan out and collect" case and
  loses the type `Task<T>` that lets the typechecker reject premature reads.
- **Collect-all-outcomes as the default** (a `List<Result<T, E>>`, no
  auto-cancel). Rejected as the *default*: the dominant backend case is "all must
  succeed or the request fails"; making the safe-but-verbose mode default taxes
  the common path. It survives as an explicit opt-in (future KDR).
- **Cancellation as a thrown/panic-like unwind.** Rejected: collides with
  KDR-0005 (panics are uncatchable, errors are values). A timeout is an expected
  condition a caller routinely handles, so it must travel the `Result`/`catch`
  channel, not the panic channel.
- **Explicit deadline/context parameter threaded through signatures (Go style).**
  Rejected: it is precisely the "coloring-by-another-name" plumbing KDR-0002
  removed; deadlines inherit ambiently down the scope tree instead.
- **Preemptive cancellation.** Rejected for now: requires runtime support the Go
  backend does not expose safely, and cooperative checkpoints at every blocking
  op cover I/O-bound backend/agent workloads (the lane, per
  [`KDR-0021`](0021-positioning.md)). Named in the reopening clause.

## Consequences

- Spec chapter 09 can now be written: it transcribes this mechanism, fixes the
  exact `scope`/`spawn`/`Task<T>` grammar and the result-collection surface, and
  registers the concurrency diagnostics (`K07xx` band — the next free band; note
  `K09xx` is already the Core "not-in-Core" rejection band). The Core reject case
  `903-no-scope-spawn` (`K0903`) is lifted from rejection to acceptance when the
  feature lands; it is not deleted, it flips.
- Unblocks the AI-infrastructure vertical: "bounded, cancelable,
  deadline-propagating agent orchestration" ([`KDR-0024`](0024-ai-infrastructure-and-agent-positioning.md))
  is exactly fail-fast scopes with inherited deadlines, and a per-agent
  token/cost budget can later propagate on the *same* ambient channel as the
  deadline.
- Authors must place a `check_cancel()` in long compute-only loops to remain
  cancelable; this is a documented, lintable obligation, not a silent footgun.
- No detached/background tasks exist — daemons are scopes owned by `main`
  (restates KDR-0002, now mechanized).
- Determinism: the *reported* error and the *order* of collected results are
  deterministic (spawn order); task execution interleaving is not, and no program
  may observe it through Keel surface.

## Reopening clause  *(required)*

Reopen if corpus evidence from real Keel concurrent/agent codebases shows
**either**:

1. that a measured majority of `scope` uses immediately opt into the deferred
   "collect-all-outcomes" mode — evidence the fail-fast default is the wrong
   default and should be inverted or made explicit; **or**

2. a demonstrated class of backend/agent workloads that cooperative
   checkpoint-based cancellation cannot cancel within an agreed bound (a named
   latency or deadline-overrun metric fixed in the reopening proposal), with no
   workaround via `check_cancel()` placement or FFI — making preemptive
   cancellation necessary.

Advocacy, "language X preempts," and benchmark microcases are never sufficient.
