---
description: Audit the agent harness for drift and promote lessons into checks
argument-hint: optional focus, e.g. "permissions" or "stale rules"
---
Audit and improve the agent harness. Optional focus: $ARGUMENTS

The harness layers are described in the root `AGENTS.md` ("The agent harness")
and enforced by `scripts/check-harness.sh`. This command is the scheduled half
of the improvement loop: it keeps the harness up with the repo instead of
letting it rot.

1. Gather evidence, not vibes: recently merged PRs and their review comments,
   CI failures on `main`, open issues labeled `harness`, and corrections
   received in the current session. Every finding must cite one of these.
2. Look for, in priority order:
   - **Prose rules that were still violated** — promote to the strongest layer
     that fits: a `scripts/check-*.sh` wired into `scripts/preflight.sh` and CI
     (covers every agent) > `.agents/settings.json` permissions/hooks (Claude
     Code) > a rule line in the nearest `AGENTS.md` (weakest).
   - **Stale guidance** — rules, paths, milestone references, or command steps
     that no longer match the repo.
   - **Grown areas without a nested `AGENTS.md`** — add one, symlink
     `CLAUDE.md -> AGENTS.md`, register it in `scripts/check-harness.sh`.
   - **Friction** — commands agents repeatedly need that are missing from the
     `.agents/settings.json` allowlist, or slash commands whose steps agents
     keep working around.
3. Sweep code debt too: `TODO`/known-ceiling comments in `compiler/` that have
   outgrown their ceiling become issues, never silent drive-by rewrites.
4. Each improvement is its own harness PR (root `AGENTS.md`, hard rule 1). Run
   `scripts/check-harness.sh`, then `scripts/preflight.sh`, before declaring
   done.
5. Nothing found? Say so and close the audit issue with a one-line result — no
   makework changes.
