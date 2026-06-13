# KDR-0021: Positioning and scope discipline

- **Status:** accepted
- **Date:** 2026-06-13
- **Scope:** governance

## Decision

A permanent "Who Keel is not for" document ships next to the tutorial and is
maintained as a normative reference. Keel is explicitly wrong for:

- Game engines, kernels, embedded targets, GUI applications, scientific
  computing
- Any domain requiring deterministic sub-100µs latency or manual memory
  control

Expressiveness requests that conflict with the primary optimisation target
("the team across five years, not the individual across five hours") are
answered by KDR link, not by negotiation. The design-positioning document is
referenced in the RFC template, and every language feature proposal must state
how it serves or conflicts with the stated niche.

## Context

Derived from [`docs/vision.md`](../vision.md) §10.

Every language that promised universality diluted itself. Every language that
named its lane and stayed in it (SQL, Erlang, Go in its original form) is
still there. The pressure to add features grows with the userbase — this KDR
makes the response structural: a feature that serves game-engine ergonomics at
the cost of team-readability is not "nice to have," it is out of scope by
definition.

This is not about excluding use cases; it is about providing a principled,
non-negotiable answer to "why doesn't Keel have X" without relitigating the
language's purpose every time.

## Alternatives considered

- **Implicit scope** ("we just won't add those features"). Rejected: without a
  written boundary, every feature request becomes a negotiation. The KDR link
  is the project's immune system.
- **No positioning document** ("let the code speak"). Rejected: the code
  doesn't speak to the 10×-engineer who wants to add dependent types to a
  backend language.
- **Periodic repositioning** ("we'll update the niche as the language grows").
  Rejected: scope creep is a death by a thousand cuts. If the corpus
  consistently shows Keel being used outside its stated niche, the *language*
  has changed and a new KDR documents the shift.

## Consequences

- The relitigation tax is eliminated: every "why no X" thread has a one-link
  answer. This frees maintainer attention for implementation.
- Some excellent engineers will be bored by Keel. This is the design working.
- Proposals for features outside the niche are rejected without detailed review
  — saving reviewer time. A feature that genuinely broadens the niche requires
  a superseding KDR first.

## Reopening clause

Corpus evidence that the stated niche is either (a) too narrow to sustain a
healthy ecosystem or (b) routinely ignored by the actual userbase (measured:
proportion of production Keel deployments outside the stated domain, with a
threshold defined in the positioning document itself).
