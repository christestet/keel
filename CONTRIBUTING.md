# Contributing to Keel

## Where to start

Read `docs/vision.md`, then `ROADMAP.md`, then pick an issue labeled with the
current milestone. If you use an LLM/agent for any part of your contribution,
`AGENTS.md` applies to you too — you are responsible for your agent's output.

## How decisions are made

Design questions are settled by **Keel Decision Records** (`docs/kdr/`), not by
PR comment threads. Existing KDRs are settled; each contains a reopening clause
stating what evidence would reopen it. To propose a design change: open an issue
titled `KDR proposal: ...` using the template in `docs/kdr/0000-template.md`.
"Language X does it differently" is not evidence; corpus data, measured build
times, and demonstrated bug classes are.

## PR rules (humans and agents alike)

- One concern per PR; spec / conformance-tests / compiler never mix.
- Run `scripts/preflight.sh` before pushing — it is exactly what CI runs.
- Compiler PRs: conformance suite green, summary pasted in description.
- Spec PRs: must state which conformance tests will encode the change.
- New compiler dependencies require explicit justification.
- Be kind in review. Critique code and evidence, never people.

## Project phase

Pre-1.0, the project is run by the founding maintainers with a stated intent to
move to foundation governance (vision.md §2). Until then: maintainers arbitrate,
KDRs bind everyone including maintainers.
