# KDR-0102: First backend compiles to Go; native backend later

- **Status:** accepted
- **Scope:** toolchain

## Decision
keelc's first backend lowers Keel IR to generated Go source and drives the Go
toolchain. A native backend (LLVM or cranelift) replaces it before 1.0. The
conformance suite is the equivalence proof between backends.

## Context
Keel's runtime semantics (concurrent low-latency GC, scheduler-managed blocking,
static cross-compiled binaries, cgroup awareness) are a near-exact description
of the Go runtime. Emitting Go buys years: Keel programs become *runnable* at M3
instead of after a multi-year GC+scheduler project. Precedent: early TypeScript
(emit JS), Kotlin (JVM first), Nim/early-Zig phases (C emission).

## Alternatives considered
LLVM first (rejected: must build GC + scheduler + linker story before "hello
world" serves HTTP). Interpreter first (rejected: performance story untestable;
double work). Transpile to Rust (rejected: impedance mismatch — would need to
fight the borrow checker from a GC language).

## Consequences
Temporary: Go toolchain is a build dependency; perf ceiling is Go's; some arena
semantics are emulated. Risk owned: `arena` (M7) and capability enforcement are
compile-time features and do not depend on the backend. Sunset: this KDR is
superseded by the native-backend KDR before 1.0 by definition.

## Reopening clause
Not applicable (self-sunsetting).
