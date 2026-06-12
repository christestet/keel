---
description: Run the executable definition of done (exactly what CI runs)
---
Run `scripts/preflight.sh` from the repo root and report the result.

If it fails, fix the failures and re-run until green — but never by weakening a
conformance expectation, editing an `expected.*` file, or skipping a stage. If
a conformance case itself seems wrong, stop and report it per the prime
directive in AGENTS.md instead of editing it.

Include the conformance runner's summary line (`N passed, N failed, N skipped`)
in your final report; compiler PRs must paste it into the PR description
(AGENTS.md, hard rule 2).
