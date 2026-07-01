# M8 reference performance corpus

This directory defines the public performance fixture for
[`KDR-0019`](../../../docs/kdr/0019-compile-time-contract.md) and the M8 query
core work tracked in [`docs/m8-status.md`](../../../docs/m8-status.md). It is a
fixture, not a conformance suite: it measures compiler latency and regression
risk without defining language behavior.

The corpus is generated deterministically by
[`scripts/m8-benchmark.sh`](../../../scripts/m8-benchmark.sh). The default shape
is a single M7 service-like module with 7,200 handler slices and more than
100,000 lines of source. Each slice declares request/response structs and a
typed handler so parsing, declaration collection, field access, struct
construction, arithmetic, control flow, and local inference are all exercised.

Run the fast editor-path measurement:

```sh
scripts/m8-benchmark.sh --mode check
```

Run the full M8 budget fixture:

```sh
scripts/m8-benchmark.sh --mode full
```

The script writes `target/m8-reference-metrics.tsv` with:

```text
metric	elapsed_ms	budget_ms	baseline_ms	status
```

`baseline.tsv` carries the KDR-0019 budgets and starts with `0` baselines. The
M8 gate PR must replace those zeros with measurements from the
[reference machine](reference-machine.md) and run the script with `--enforce`.
Until then, this fixture is informational and is intentionally not wired into
CI.
