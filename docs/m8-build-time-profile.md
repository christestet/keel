# M8 build-time profile — where `keel_build_cold` spends its time

Non-normative profiling note. It answers the open question left in
[`m8-status.md`](m8-status.md) ("`keel_build_cold`'s 18.8 s against a 10 s
budget needs its own investigation — profiling hasn't started") and feeds
M8a's slice-4 gate PR. Budgets and the reference machine are defined by
[KDR-0019](kdr/0019-compile-time-contract.md) and
[`ROADMAP.md`](../ROADMAP.md) §M8; the source of truth for reference-machine
numbers is the `m8-benchmark` job log, not this note.

## Status

Profiling done for the three KDR-0019 metrics on the reference corpus
(`scripts/m8-benchmark.sh`, `M8_REFERENCE_HANDLERS=7200`). No compiler or
benchmark behavior changed as part of the profile. Conclusions below are the
input to slice 4 (the gate PR), which is still not started.

## What the corpus actually compiles

The 7,200-handler corpus is ~43K lines of Keel. keelc lowers it to a **single
Go package / single `main.go` of ~115K lines (2.95 MB)** — 7,200 `fn`s plus
14,400 `struct`s (`Request####`/`Response####`). `keel build` then shells out
to `go build -trimpath -buildvcs=false` (hermetic build, spec ch.18 /
KDR-0105) on that one package.

## Measured breakdown

Reference-machine totals (`baseline.tsv`, run 28533408054, 2 vCPU) are the
gate's source of truth:

```text
keel_check              794 ms   (budget 300)
keel_build_cold       18770 ms   (budget 10000)
keel_build_incremental 1701 ms   (budget 1000)
```

The phase split below was measured in a 4-core coding-agent sandbox (so it is
~2× faster than the 2-vCPU reference machine) — it characterizes *where* the
time goes, not the gate numbers:

| Phase | Sandbox time | What it is |
| --- | --- | --- |
| `keel check` (parse→resolve→typecheck→diagnostics) | ~0.6 s | keelc's own front-end work |
| `keel build`, `GOCACHE` cold | ~13 s | ~95% is the `go build` subprocess |
| `keel build`, `GOCACHE` warm, source unchanged | ~1.4 s | everything cached |
| `go build`: compile Go std-lib deps (cold) | ~4–9 s | one-time per cold cache |
| `go build`: compile the 115K-line generated package | ~5.8 s | recompiled whenever codegen output changes |
| `go build`: same corpus at 400 handlers (6.5K lines) | ~0.46 s | compile scales with package size |
| `go build`: fully warm, identical source | ~0.2 s | pure cache hit |

## Findings

1. **keelc is not the bottleneck for `keel_build_cold`.** Its whole front-end
   runs in ~0.6 s; the rest of the wall-clock is the `go build` subprocess.
   The one budget that is genuinely keelc's own hot path is `keel check`
   (794 ms vs 300 ms).
2. **The `m8-benchmark` CI job never persists or pre-warms `GOCACHE`.** It
   caches cargo via `Swatinem/rust-cache` but nothing for Go, so every run
   recompiles the Go standard-library dependency graph from scratch (~4–9 s in
   sandbox; more on 2 vCPU). This is repeated, cacheable work.
3. **`-trimpath` uses a separate build-cache namespace from a non-trimpath
   cache.** Verified: a cold build without `-trimpath` followed by one with
   `-trimpath` recompiled everything twice. A Go cache pre-populated by an
   unrelated build does not help the hermetic build.
4. **The corpus emits one monolithic Go package.** A single package compiles
   largely serially and cannot fan out across CPUs, so it costs ~5.8 s in
   sandbox (12.4 s of CPU) and recompiles on every codegen-changing PR — which
   is exactly when `m8-benchmark` runs. Compile time scales with package size
   (400 handlers = 0.46 s, 7,200 = 5.8 s).

So the reference-machine 18.8 s ≈ cold std-lib compile + monolithic-package
compile, and the KDR-0019 "cold build < 10 s" contract — meant to gate
keelc — is mostly measuring the Go toolchain bootstrap.

## Dependency chain

- [KDR-0019](kdr/0019-compile-time-contract.md) — the budgets and the
  reference-machine reasoning being measured here.
- [`m8-status.md`](m8-status.md) — M8a slices; this note supplies slice 4's
  missing profiling input.
- [`tests/performance/m8-reference/`](../tests/performance/m8-reference/README.md)
  — corpus, reference machine, and `baseline.tsv`.
- [`scripts/m8-benchmark.sh`](../scripts/m8-benchmark.sh) — the runner and the
  `-trimpath` / `-buildvcs=false` build flags.
- [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) — Go-backend
  pipeline and the single-package emission this note profiles.

## Milestone boundary

Slice 4 (turning on `--enforce`) is still gated on closing the budget gap.
Two of the levers below are compiler/codegen-scope and must go through their
own KDR/issue, not a drive-by change to the corpus or backend.

## Next work

- **CI (harness concern, low-risk):** cache/pre-warm `GOCACHE` in the
  `m8-benchmark` job (`actions/cache` keyed on Go version + a
  `go build -trimpath std` warm step) so cold runs stop recompiling the Go
  std lib. Caveat: on codegen-changing PRs the generated package still
  recompiles (~6 s sandbox, likely 10 s+ on 2 vCPU), so caching alone may not
  clear 10 s on the reference machine.
- **keelc (in M8a scope):** profile and reduce `keel check`'s hot path
  (794 ms → < 300 ms). This is the KDR-0019 budget that actually measures
  keelc.
- **Decision needed (KDR/issue, not a quiet edit):** whether the cold-build
  budget should exclude Go toolchain bootstrap, and whether keelc should emit
  multiple Go packages (or the corpus be right-sized away from 7,200
  near-identical handlers in one package) so `go build` parallelizes. This is
  codegen scope.
- **Already tracked:** real `keel_build_incremental` needs per-declaration
  query granularity — deferred as a `--known-gap` in `m8-status.md`.

## Validation snapshot

Reproduce the phase split:

```sh
cargo build --release -p keelc-driver
M8_REFERENCE_HANDLERS=7200 scripts/m8-benchmark.sh --mode full --known-gap keel_build_incremental
# isolate the go-build phase: capture the emitted main.go, then
go clean -cache && time go build -trimpath -buildvcs=false -o /dev/null main.go   # cold
time go build -trimpath -buildvcs=false -o /dev/null main.go                      # warm
```

The reference-machine totals remain those recorded in `baseline.tsv` and the
`m8-benchmark` job log.
