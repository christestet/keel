# capability-audit — the supply-chain differentiator

A third example for the 0.1.0 developer preview, alongside
[`hello.keel`](../hello.keel) (M3), [`users-service`](../users-service/README.md)
(M6/M7), and [`job-pipeline`](../job-pipeline/README.md) (M6). This one is not
about a language feature — it's about the question every backend/platform
engineer asks about a dependency and today can't answer without reading its
source: **what can this thing actually reach?**

## The workspace

A small multi-package build ([`06-modules-packages.md`](../../docs/spec/06-modules-packages.md),
[`11-capabilities.md`](../../docs/spec/11-capabilities.md)):

```
checkout_service/     capabilities = ["net"]     (this package)
  payments/            capabilities = ["net"]     path dependency — reaches a payment gateway
  pricing/              capabilities = []          path dependency — pure arithmetic
```

`payments` declares `net` because it uses `std.http`. `pricing` declares
nothing — it's provably harmless. `checkout_service` must declare `net` too,
**even though it never calls `std.http` itself**, because the manifest's
transitive rule is `declared(dependent) ⊇ declared(dependency)`: authority a
dependency holds is authority the dependent build can reach, so it can't be
smuggled in silently.

## What's implemented vs. what isn't (read this before extending the demo)

Manifest capability declaration, static enforcement (`K1110`/`K1112`), and
`keel audit` reporting are what this example exercises. Cross-package function,
type, and interpolated calls now **do** link into the executable (KDR-0044; see
`docs/cross-package-linking-notes.md`), but this example stays deliberately
narrow: `main.keel` only `use`s `payments.charge` and `pricing.quote` to put
them in the dependency and capability graph the audit reasons over, without
calling them. That keeps the demo about capabilities, not linking — for a
runnable cross-package call, see conformance `818`/`838`.

## Run it

```sh
./target/release/keel run examples/capability-audit/main.keel --milestone M7
# [info] checkout service ready

./target/release/keel audit examples/capability-audit/main.keel --milestone M7
```

```text
checkout_service 0.1.0
  net: self, payments 0.1.0
  (fs, exec, env, ffi, unsafe-memory: not present)
```

One command answers "which of my dependencies can open a socket?" — computed
statically from manifests and the call graph, without running the program
([spec §11.5](../../docs/spec/11-capabilities.md#115-keel-audit)).

## See the enforcement fire

Both of these are real compiler output, captured against this workspace and
reverted before committing — try them yourself:

Drop `net` from `checkout_service/keel.toml` while `payments` still declares
it:

```text
error[K1112]: package `checkout_service` depends on `payments` which requires `net`; declare it too
```

Drop `net` from `payments/keel.toml` while it still `use`s `std.http`:

```text
error[K1110]: package `payments` uses `std.http` which requires capability `net`; declare it
```

Both are compile-time diagnostics from reading manifests and source text —
never panics, never a runtime check ([hard rule 6](../../AGENTS.md)).

Like the other examples, this is not a conformance case: behavior guarantees
live in [`tests/conformance/`](../../tests/conformance/), not here.
