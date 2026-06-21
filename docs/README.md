# Documentation

Required reading order (from [`AGENTS.md`](../AGENTS.md)):

1. [`vision.md`](vision.md) — language and tooling design rationale
2. [`spec/keel-core.md`](spec/keel-core.md) — the frozen M0–M4 language subset (normative)
3. [`spec/07-interfaces.md`](spec/07-interfaces.md) — first post-Core chapter: nominal interfaces (normative)
4. [`spec/08-generics.md`](spec/08-generics.md) — interface-constrained generics (normative, parser scaffolding complete)
5. [`kdr/INDEX.md`](kdr/INDEX.md) — decision records (KDRs), accepted and rejected
6. [`compiler/ARCHITECTURE.md`](../compiler/ARCHITECTURE.md) — compiler pipeline, crate layout, iron rules

## Directory map

| Path | Purpose |
|---|---|
| [`vision.md`](vision.md) | Design document v0.2 — the "why" behind every feature and decision. Start here. |
| [`spec/`](spec/) | Normative language specification. `keel-core.md` is the frozen subset; numbered chapters add tested detail. |
| [`kdr/`](kdr/) | Keel Decision Records — every adopted or rejected design decision, with reopening clauses. |
| [`milestone-status.md`](milestone-status.md) | Non-normative implementation status per roadmap milestone. |
| [`generics-implementation.md`](generics-implementation.md) | Implementation tracking for interface-constrained generics (M5). |
| [`m6-simplification-audit.md`](m6-simplification-audit.md) | M6 audit decisions, `?` span invariant, semantic deduplication, and validation snapshot. |
| [`ROADMAP.md`](../ROADMAP.md) | Milestones with exit criteria. |
