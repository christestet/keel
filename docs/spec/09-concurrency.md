# 09 — Concurrency (scope / spawn)

This chapter is **normative**. It adds **structured concurrency** to the
language: the `scope`/`spawn` model decided in shape by
[`KDR-0002`](../kdr/0002-no-async-await.md) and in mechanism by
[`KDR-0026`](../kdr/0026-structured-concurrency-mechanism.md). It does not
restate the frozen rules in [`keel-core.md`](keel-core.md); on any conflict with
`keel-core.md`, file an issue rather than reconciling silently (the prime
directive in the root [`AGENTS.md`](../../AGENTS.md)).

Implementation status: **partially implemented for M5.** The compiler accepts
`scope`/`spawn`, types `Task<T>`, enforces `K0701`-`K0703`, lowers through KIR,
and emits Go for join-barrier execution and fail-fast `Result` propagation.
The Core reject case `903-no-scope-spawn` is milestone-gated through M4.
`scope(deadline: ...)` parses and lowers, but Go backend emission for deadlines
is intentionally still unsupported until the M6 time/duration stdlib surface
exists. Cancellation checkpoint APIs such as `check_cancel()` are likewise
specified here but not exposed by the current stdlib slice.

Structured concurrency in Keel is intentionally rigid:

- Concurrency is expressed only inside a `scope` block. There are no detached or
  background tasks; a daemon is a `scope` owned by `main` (KDR-0002).
- A task cannot outlive the `scope` that spawned it. The closing brace is a join
  barrier.
- Failure is fail-fast by default; cancellation is cooperative and travels the
  ordinary `Result` channel, never the panic channel (KDR-0005).

## 9.1 Scopes and the join barrier

A `scope` block delimits a task tree:

```keel
scope {
    spawn work()
}   // join barrier: control does not pass this brace until work() has terminated
```

The closing brace is an **implicit join barrier**. Control does not pass it until
every task spawned in the scope has terminated — by completing, by failing, or by
finishing the unwinding of a cancellation. There is no syntax that detaches a
task from its scope, so the orphaned-goroutine class is unrepresentable.

A `scope` is an **expression** (`keel-core.md` §4: everything is an expression).
Its value is defined in §9.3.

## 9.2 `spawn` and `Task<T>` handles

Inside a `scope` body, `spawn e` launches a child task that evaluates the
expression `e`. The `spawn` expression has type `Task<T>` where `e: T`:

```keel
scope {
    let a = spawn fetch_profile(id)   // a : Task<Profile>
    ...
}
```

`spawn` outside any enclosing `scope` is a compile error (`K0701`). A `Task<T>`
handle may not escape its scope — returning it, or storing it in a binding that
outlives the scope, is `K0703`. This makes the type non-leakable by construction
and keeps the scope's allocation region
([`KDR-0016`](../kdr/0016-scope-implicit-arenas.md)) sound: nothing the region
backs can be observed after the barrier frees it.

The body's non-tail statements execute sequentially on the scope's own task,
launching children as control reaches each `spawn`; children run concurrently
with one another and with later statements. Execution interleaving is not
observable through any Keel surface (determinism — root `AGENTS.md` hard rule 7).

## 9.3 The post-join tail and reading results

The **tail** of a scope is its final expression (the block value, `keel-core.md`
§4). The tail is evaluated **after** the join barrier. A handle's result is read
with `.value`, and `.value` is well-typed **only in the tail** — reading it in
any earlier statement is `K0702`, because the result does not exist until the
barrier has resolved.

```keel
let user = scope {
    let a = spawn fetch_profile(id)
    let b = spawn fetch_prefs(id)
    User { profile: a.value, prefs: b.value }   // tail: runs after both join
}
```

The value of an infallible handle `h: Task<T>` is `h.value: T`. The scope
expression's value is the value of its tail. A scope whose tail is a statement of
type `Unit` (a pure-effect scope) has type `Unit`.

## 9.4 Fail-fast and fallible tasks

A spawned task is **fallible** if its expression type is `Result<U, E>`, and
**infallible** otherwise.

- If a scope contains at least one fallible task, the scope expression has type
  `Result<R, E>`, where `R` is the type of the tail and `E` is the union
  (`keel-core.md` §5) of the error types of the scope's fallible tasks.
- If a scope contains no fallible task, the scope expression has type `R`.

At the barrier, if every fallible task produced `Ok`, the tail is evaluated and
the scope yields `Ok(tail)`. If any fallible task produced `Err`, the scope
**cancels all sibling tasks**, waits for them to unwind, does **not** evaluate the
tail, and yields that `Err`. When more than one fallible task has failed by the
time the barrier resolves, the propagated error is the one from the
**lowest-indexed task in spawn order** (deterministic selection — hard rule 7).

Because the tail runs only when all fallible tasks succeeded, a fallible handle
`h: Task<Result<U, E>>` exposes its **success payload** directly in the tail:
`h.value: U`. The scope therefore composes with `?`:

```keel
fn load(id: Id) -> Result<User, DbError> {
    scope {
        let a = spawn fetch_profile(id)   // Task<Result<Profile, DbError>>
        let b = spawn fetch_prefs(id)     // Task<Result<Prefs, DbError>>
        User { profile: a.value, prefs: b.value }   // a.value : Profile
    }   // scope : Result<User, DbError>; fail-fast on the first DbError
}
```

The non-fail-fast "collect every outcome" mode named in KDR-0026 is **not** part
of this chapter; it requires a future KDR and is not yet expressible.

## 9.5 Cancellation

Cancellation is **cooperative** and **value-based**. It is delivered at
**checkpoints**: every runtime blocking operation (I/O, channel operations,
sleep, a nested join barrier) is a checkpoint, plus an explicit `check_cancel()`
for compute-bound loops that would otherwise never block:

```keel
scope {
    spawn {
        while true {
            check_cancel()       // cooperative checkpoint
            step()
        }
    }
}
```

A cancelled operation returns the built-in error `Cancelled` through the ordinary
`Result` channel, so cleanup runs through the normal scope-exit and `catch`
(`keel-core.md` §5) paths. `Cancelled` is a built-in error type introduced by
this chapter.

Cancellation is **never a panic**. Panics (KDR-0005) remain uncatchable and
orthogonal: a panicking task aborts the process; it does not become a recoverable
cancellation. A task that ignores every checkpoint cannot be cancelled — placing
a `check_cancel()` in a long compute-only loop is a documented, lintable
obligation, not a silent default.

## 9.6 Deadlines

A `scope` may carry a deadline, after which its still-running tasks are cancelled
via the §9.5 path and the scope yields `Cancelled`:

```keel
scope(deadline: d) {
    spawn slow_call()
}
```

Deadlines are **ambient and monotone**:

- The deadline propagates implicitly to every descendant task and nested scope.
  It is **never threaded through function signatures** — KDR-0002 removed
  `context.Context`; cancellation and deadlines are ambient.
- A nested scope may only **tighten** an inherited deadline. The effective
  deadline of any scope is the minimum of its declared deadline and the inherited
  one; a declared deadline later than the inherited one has no effect.

The concrete duration/instant value type and its literal surface (e.g.
`2.seconds`) land with the standard library slice (M6) and are specified there;
this chapter fixes only the propagation and cancellation **semantics**, which do
not depend on that surface.

## 9.7 Capabilities and memory

A spawned task runs under the **capability budget of the enclosing function**
([`KDR-0017`](../kdr/0017-function-capabilities.md)); `spawn` introduces no new
authority. The scope's implicit allocation region
([`KDR-0016`](../kdr/0016-scope-implicit-arenas.md)) is freed at the join
barrier — sound precisely because the barrier guarantees no child outlives the
scope, which §9.2's `K0703` enforces. These two chapters (11, 10) govern the
respective rules; this chapter only states the interaction.

## 9.8 Examples (normative, extracted by CI)

```keel
struct User {
    profile: Profile
    prefs: Prefs
}

struct Profile { name: String }
struct Prefs { theme: String }

fn fetch_profile(id: Int) -> Result<Profile, DbError> {
    Ok(Profile { name: "user-{id}" })
}

fn fetch_prefs(id: Int) -> Result<Prefs, DbError> {
    Ok(Prefs { theme: "dark" })
}

fn load(id: Int) -> Result<User, DbError> {
    scope {
        let a = spawn fetch_profile(id)
        let b = spawn fetch_prefs(id)
        User { profile: a.value, prefs: b.value }
    }
}

fn main() -> Unit {
    let user = load(1) catch err {
        other => return,
    }
    print(user.profile.name)
}
```

## 9.9 Error conditions

The following are errors with stable `K####` codes, registered in the
accompanying registry PR (`K07xx` is the next free band; `K09xx` is the Core
"not-in-Core" rejection band):

- **`K0701` — `spawn` outside a `scope`.** A `spawn` expression appears with no
  enclosing `scope` block.
- **`K0702` — task result read before the join barrier.** A handle's `.value` is
  read anywhere other than the scope's tail expression (§9.3).
- **`K0703` — task handle escapes its scope.** A `Task<T>` handle is returned
  from, or bound outside, the scope that produced it.

Malformed `scope`/`spawn` syntax is reported as a syntax error under the existing
code `K0003`.

## 9.10 Conformance cases this chapter introduces

The initial M5 conformance slice is landed in cases `710`-`715`. Deadline and
explicit cancellation-checkpoint cases are deferred until the M6 stdlib surface
provides the necessary time/cancellation APIs.

| Case | Kind | Asserts |
|---|---|---|
| `710-scope-spawn-join` | accept | a single `spawn` joins at the barrier; tail reads `.value` |
| `711-scope-two-spawn-fanout` | accept | two fallible spawns; tail builds a struct from both payloads |
| `715-scope-fail-fast` | accept | first `Err` cancels siblings; scope yields that `Err` (lowest spawn index) |
| `712-spawn-outside-scope` | reject `K0701` | `spawn` with no enclosing `scope` |
| `713-task-value-before-join` | reject `K0702` | `.value` read in a non-tail statement |
| `714-task-handle-escapes` | reject `K0703` | a `Task<T>` returned from its scope |
| deferred `scope-cancel-catch` | accept | a `Cancelled` error is handled by `catch` once cancellation APIs exist |

## 9.11 Dependencies

- Decisions: [`KDR-0002`](../kdr/0002-no-async-await.md) (shape),
  [`KDR-0026`](../kdr/0026-structured-concurrency-mechanism.md) (mechanism).
- Related: [`KDR-0016`](../kdr/0016-scope-implicit-arenas.md) (scope regions),
  [`KDR-0017`](../kdr/0017-function-capabilities.md) (capability inheritance),
  [`KDR-0024`](../kdr/0024-ai-infrastructure-and-agent-positioning.md) (the agent
  orchestration this chapter underpins).
- Frozen base: [`keel-core.md`](keel-core.md) §1 (keywords `scope` and `spawn`
  are reserved for later milestones), §4 (expressions, block value), §5 (errors,
  `Result`, `catch`, union error types).
- Code registry: `K0701`–`K0703` are registered in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  (append-only) ([`docs/spec/AGENTS.md`](AGENTS.md)).
