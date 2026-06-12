---
description: Write or update a standalone implementation wiki note under docs/
argument-hint: topic to document, e.g. "M1 compiler workspace"
---
Create or update a standalone wiki-style note for: $ARGUMENTS

Use this for implementation status, orientation, dependency maps, and "what
happened / what comes next" notes. Do not use it for normative language
changes; specs, KDRs, conformance tests, and compiler changes remain separate
PR concerns.

Requirements:

1. Put the note under `docs/` with a narrow kebab-case filename.
2. Link governing docs instead of restating them: `AGENTS.md`, `ROADMAP.md`,
   relevant spec sections, KDRs, architecture docs, and conformance docs.
3. Include explicit sections for status, not-done-yet, dependency chain,
   milestone boundary, validation snapshot, and next work when they apply.
4. Keep the note non-normative. If documenting new behavior requires changing
   spec, conformance, or compiler code, stop and split the work into the
   required PR sequence.
5. Run `scripts/preflight.sh` before declaring done.
