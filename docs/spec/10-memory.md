# 10 — Memory (GC + scoped arenas)

This chapter is **normative**. It defines Keel's memory model: a concurrent
garbage collector for general allocation, plus lexically scoped `arena` blocks
that bypass the GC for hot paths. The model and its rejected alternatives are
[`KDR-0012`](../kdr/0012-gc-plus-scoped-arenas.md) (GC + arenas, no
ownership/lifetimes) and [`KDR-0016`](../kdr/0016-scope-implicit-arenas.md)
(every `scope` is also a region). It does not restate the frozen rules in
[`keel-core.md`](keel-core.md); on any conflict, file an issue rather than
reconciling silently (the prime directive, root [`AGENTS.md`](../../AGENTS.md)).

Implementation status: **specified, not yet implemented.** The `arena` keyword
is reserved ([`keel-core.md`](keel-core.md) §1) and rejected outside its
milestone by the Core case `904-no-arena` (`K0904`). This chapter governs the
M7 arena work.

Keel has **no manual `free`, no lifetime annotations, and no ownership/borrowing
model** ([`KDR-0012`](../kdr/0012-gc-plus-scoped-arenas.md)). Memory safety is a
guarantee, not a discipline the programmer maintains.

## 10.1 General allocation: the garbage collector

All allocation that is not proven region-local is served by a concurrent,
low-latency garbage collector. The GC is not observable through any Keel surface
(no finalizers, no `gc()` call, no weak references in this milestone), so its
presence never changes program meaning — only timing, which determinism does not
constrain ([`AGENTS.md`](../../AGENTS.md) hard rule 7 governs *output*, not
wall-clock).

GC pacing may be tuned **at build time**, not through a runtime tuning surface:
`keel build --profile latency` and `--profile throughput` select pacing
profiles, and the runtime reads cgroup limits for container-awareness
([`KDR-0012`](../kdr/0012-gc-plus-scoped-arenas.md)). These flags affect timing
only, never observable behavior.

## 10.2 `arena` blocks

An `arena` block is a **lexically scoped region**. Allocations performed inside
it are served from the region and freed in O(1) when the block exits — the GC
never traces them:

```keel
arena {
    let nodes = parse(input)   // every allocation here lands in the arena
    summarize(nodes)
}   // the whole region is freed in O(1); the GC never saw it
```

`arena` has the **same shape as `scope`** ([`09-concurrency.md`](09-concurrency.md)):
lexical, visible, impossible to leak, and statically checked. It introduces no
new conceptual machinery — the check in §10.3 is the same escape analysis the GC
already performs, pointed at one new rule.

An `arena` is an **expression** ([`keel-core.md`](keel-core.md) §4). Its value is
the value of its tail, subject to §10.3: a tail value whose representation is
backed by region memory cannot leave the block.

## 10.3 The escape rule

> A reference to a value allocated in an `arena` block may not outlive that
> block.

The compiler's escape analysis enforces this statically. A region-backed
reference that is returned from the block, stored in a binding that outlives it,
captured by a task or closure that outlives it, or yielded as the block's value,
is **`K1001`**. Because the rule is checked at compile time, the
use-after-free and dangling-region classes are **unrepresentable**, not merely
discouraged.

Values whose representation does not reference region memory — primitives, and
copies — pass freely out of the block. A program that needs an arena-built graph
to survive the block allocates it under the GC instead (omit the `arena`); the
diagnostic names this remedy.

```keel
fn build() -> Tree {
    arena {
        let scratch = parse(input)   // region-local working set
        scratch.to_tree()            // K1001 if to_tree() returns region-backed nodes
    }
}
```

## 10.4 Scope-implicit regions

Every `scope` block ([`09-concurrency.md`](09-concurrency.md) §9.7) is **also**
a region ([`KDR-0016`](../kdr/0016-scope-implicit-arenas.md)). Allocations the
escape analysis proves do not outlive the scope are served from the scope's
region and freed in O(1) at the join barrier; the rest go to the GC. The
programmer writes nothing — the §10.3 analysis applies to `scope` automatically.
This is sound precisely because the join barrier guarantees no child outlives
the scope ([`09-concurrency.md`](09-concurrency.md) §9.2, `K0703`).

Explicit `arena` blocks remain the sequential counterpart for hot paths with no
concurrency; the scope-implicit region is the concurrent one. They are the same
mechanism, not two paradigms.

## 10.5 Examples (illustrative)

```keel
struct Node { value: Int }

fn sum_tree(input: String) -> Int {
    arena {
        let nodes = parse_nodes(input)   // List<Node>, region-local
        let mut total = 0
        for n in nodes {
            total = total + n.value      // Int copies out freely
        }
        total                            // tail is an Int — not region-backed — OK
    }
}
```

## 10.6 Error conditions

Registered (append-only) by the implementation PR in the `K10xx` band:

- **`K1001` — arena reference escapes its block.** A value backed by `arena` (or
  scope-implicit, §10.4) region memory is returned from, bound outside, captured
  beyond, or yielded as the value of, the block that allocated it.

Use of `arena` before its milestone remains the Core rejection `K0904`
(`904-no-arena`). Malformed `arena` syntax is the existing syntax error `K0003`.

## 10.7 Conformance cases this chapter introduces

| Case | Kind | Asserts |
|---|---|---|
| `830-arena-local-allocation` | accept | allocations inside `arena` compile; a primitive tail value leaves the block |
| `831-arena-frees-region` | accept | an `arena` block runs and produces its (non-region-backed) result |
| `832-arena-reference-escapes` | reject `K1001` | a region-backed reference returned from the block |
| `833-scope-implicit-region` | accept | a `scope` whose non-escaping allocations are region-served (observably identical to GC) |

## 10.8 Dependencies

- Decisions: [`KDR-0012`](../kdr/0012-gc-plus-scoped-arenas.md) (GC + arenas, no
  ownership), [`KDR-0016`](../kdr/0016-scope-implicit-arenas.md) (scope-implicit
  regions).
- Related chapters: [`09-concurrency.md`](09-concurrency.md) §9.7 (the scope/
  region boundary), [`11-capabilities.md`](11-capabilities.md) (`unsafe-memory`
  is the only escape from these guarantees).
- Frozen base: [`keel-core.md`](keel-core.md) §1 (`arena` reserved), §4
  (expressions, block value), §8 (`K0904` rejects `arena` outside its milestone).
- Code registry: `K1001` is registered (append-only) in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  by the implementation PR.
