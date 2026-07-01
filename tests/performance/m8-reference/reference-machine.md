# M8 reference machine

The M8 performance gate will use one published machine profile for baseline
capture and regression comparison. The profile is not filled in by this fixture
commit because no baseline has been captured yet.

The gate PR must record:

- CPU model and core count
- RAM size
- storage type
- operating system image and version
- Rust toolchain version
- Go toolchain version
- `keelc` commit used for the baseline
- the exact output of `scripts/m8-benchmark.sh --mode full`

Until those fields are recorded and `baseline.tsv` contains nonzero baselines,
the M8 benchmark script must stay out of required CI.
