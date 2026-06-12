# docs/kdr/ — agent rules (adds to the root AGENTS.md, never replaces it)

KDRs are settled decisions. They bind you: do not relitigate them in code,
comments, examples, or "improvements" elsewhere in the repo.

- **Never alter the Decision section of an accepted KDR.** A decision changes
  only via a new KDR that supersedes it, and only when the evidence named in
  the old KDR's reopening clause exists.
- New KDRs: copy `0000-template.md`, take the next free number in the correct
  band (00xx language/design, 01xx implementation/toolchain), fill in **every**
  section — the reopening clause is mandatory — and add a row to `INDEX.md`.
- Expanding a stub (marked in `INDEX.md`) means faithful transcription from
  `docs/vision.md` plus a reopening clause. No new design decisions may be
  smuggled into a stub expansion.
- "Language X does it differently" is never evidence. Corpus data, measured
  build times, and demonstrated bug classes are (see `CONTRIBUTING.md`).
