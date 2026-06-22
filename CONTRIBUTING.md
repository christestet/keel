# Contributing to Keel

## Where to start

Start from [`docs/README.md`](docs/README.md). Compiler contributors then read
`docs/vision.md`, `ROADMAP.md`, and `compiler/ARCHITECTURE.md` before picking an
issue labeled with the current milestone. If you use an LLM/agent for any part
of your contribution, `AGENTS.md` applies to you too — you are responsible for
your agent's output.

## How decisions are made

Design questions are settled by **Keel Decision Records** (`docs/kdr/`), not by
PR comment threads. Existing KDRs are settled; each contains a reopening clause
stating what evidence would reopen it. To propose a design change: open an issue
titled `KDR proposal: ...` using the template in `docs/kdr/0000-template.md`.
"Language X does it differently" is not evidence; corpus data, measured build
times, and demonstrated bug classes are.

## PR rules (humans and agents alike)

The PR rules are defined once in [`AGENTS.md`](AGENTS.md) ("Hard rules" and
"What done means") — they bind humans and agents identically, so they are not
restated here. The short version: one concern per PR, `scripts/preflight.sh`
green before pushing, conformance summary in the description.

One rule that is ours alone: be kind in review. Critique code and evidence,
never people.

## Project phase

Pre-1.0, the project is run by the founding maintainers with a stated intent to
move to foundation governance (vision.md §2). Until then: maintainers arbitrate,
KDRs bind everyone including maintainers.
