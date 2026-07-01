# M8 reference machine

The reference machine is the `m8-benchmark` job's own runner in
[`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml): the standard
GitHub-hosted `ubuntu-latest` Linux runner (2 vCPU, 7 GB RAM, 14 GB SSD per
[GitHub's published runner specifications](https://docs.github.com/en/actions/reference/specifications-for-github-hosted-runners)).
A contributor's laptop or an interactive coding-agent sandbox is deliberately
*not* the reference machine — KDR-0019's own reasoning is that a public,
reproducible benchmark "prevents 'works on my machine' dismissals," so the
baseline must come from the same machine class the gate runs on for every PR,
not from whatever happened to be available when the baseline was captured.

GitHub occasionally changes the exact runner image/spec, so this file names
the runner label rather than pinning numbers that would silently go stale.
Every `m8-benchmark` run re-records CPU/RAM/disk/OS/Rust/Go versions and the
`keelc` commit in its own job log — that per-run record, not this file, is the
source of truth for what actually produced a given baseline number.

## Current baseline

Captured from the `m8-benchmark` job, [workflow run
28533408054](https://github.com/christestet/keel/actions/runs/28533408054),
commit `4283835`, 2026-07-01. Machine, as self-reported by that run:

- CPU: 2 vCPU, AMD EPYC 7763 64-Core Processor
- RAM: 7.8 GiB
- OS: Ubuntu 24.04.4 LTS, kernel `6.17.0-1018-azure`
- Rust: `rustc 1.96.1 (31fca3adb 2026-06-26)`
- Go: `go1.24.13 linux/amd64`

```text
metric                   elapsed_ms  budget_ms  status
keel_check               794         300        over-budget
keel_build_cold          18770       10000      over-budget
keel_build_incremental   1701        1000       over-budget
```

All three metrics exceed their KDR-0019 budget on this run — this is a real
compiler-performance gap on the reference machine itself, not a measurement
artifact of a noisy or underpowered local sandbox. See
[`docs/m8-status.md`](../../../docs/m8-status.md) for why `--enforce` stays
off (enforcing budgets none of the three metrics currently meet would just
block every future compiler PR, not signal a new regression) and for what
closing this gap needs. `baseline.tsv` records these numbers so the 5%
regression check still catches further slowdowns from here.
