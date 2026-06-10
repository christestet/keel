# Specification plan

The full normative spec grows chapter by chapter, each landing together with its
conformance tests (spec PR → test PR → implementation PR, per AGENTS.md).

Planned chapters: 01-lexical, 02-types, 03-declarations, 04-expressions,
05-errors, 06-modules-packages, 07-interfaces, 08-generics, 09-concurrency
(scope/spawn), 10-memory (GC + arena), 11-capabilities, 12-ffi, 13-testing,
14-editions, 15-stdlib-core.

Until a chapter exists, `keel-core.md` plus the conformance suite is the only
normative text. Style: every normative statement is testable; every error gets a
stable K#### code; examples in spec chapters are extracted and run by CI
(literate-spec discipline, like the Rust reference's tested examples).
