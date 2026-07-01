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

Captured from the `m8-benchmark` job on `ubuntu-latest`, workflow run
[to be filled in], commit [to be filled in]. See
[`baseline.tsv`](baseline.tsv) for the numbers and
[`docs/m8-status.md`](../../../docs/m8-status.md) for what is and is not
enforced yet.
