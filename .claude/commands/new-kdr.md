---
description: Draft a new Keel Decision Record or expand an accepted stub
argument-hint: decision to record, or "expand stub NNNN"
---
Work on a KDR: $ARGUMENTS

Follow `docs/kdr/AGENTS.md` exactly:

- **New decision:** copy `docs/kdr/0000-template.md` to the next free number in
  the correct band (00xx language/design, 01xx implementation/toolchain). Fill
  in every section; the reopening clause is mandatory and must name specific,
  falsifiable evidence. Status starts as `proposed`. Add a row to `INDEX.md`.
  New KDRs are proposed via an issue using `.github/ISSUE_TEMPLATE/kdr-proposal.md`
  before they can be accepted — do not mark a KDR `accepted` yourself.
- **Stub expansion:** transcribe the relevant `docs/vision.md` section into the
  template faithfully — no new design decisions — and write a reopening clause
  for review. Update the stub marker in `INDEX.md`.

Never alter the Decision section of an accepted KDR, and never relitigate one.
