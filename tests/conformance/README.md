# Conformance suite — the executable spec

This directory is the ground truth for keelc. **If you are an LLM implementing
the compiler, this directory matters more than any prose.** A behavior exists
when a conformance case encodes it; otherwise it does not exist.

## Case format

Each case is a directory: `NNN-kebab-name/` containing `main.keel` plus exactly
one expectation file:

- `expected.stdout` — accept-case: program compiles, runs, stdout must match byte-for-byte (trailing newline normalized).
- `expected.error` — reject-case: first line is the diagnostic code (`K0301`),
  optional second line is `line:N` for the primary span. Message text is NOT matched.

Optional `case.toml` for flags (edition, milestone gate):

```toml
milestone = "M2"     # runner skips cases beyond the current milestone
```

## Rules

- One behavior per case. A case that tests two things is two cases.
- Reject-cases must be *minimal*: remove any line and the error disappears.
- Case numbers are permanent (like diagnostic codes). Gaps are fine; renumbering is forbidden.
- Naming: `0xx` lexical, `1xx` types, `2xx` declarations, `3xx` struct/enum/match,
  `4xx` control flow, `5xx` errors/Result, `6xx` modules, `7xx` tests/assert,
  `9xx` not-in-Core rejections.

## Runner

`cargo run -p conformance-runner` (exists from M1). Exit nonzero on any failure;
prints `pass/fail/skip` counts and a per-case diff on mismatch.

## keelc diagnostic contract (what the runner checks)

Reject-cases require keelc stderr to contain the diagnostic code (e.g. `K0301`)
and, when `line:N` is specified, the literal span `main.keel:N`. The recommended
human format (not matched beyond those substrings):

    error[K0301]: struct `User` is missing field `name`
      --> main.keel:7:13

Run locally:

    cargo run -p conformance-runner -- --check                 # structure only (pre-M1)
    cargo run -p conformance-runner -- --keelc target/release/keelc
    cargo run -p conformance-runner -- --keelc ... --milestone M3   # skip later-milestone cases
