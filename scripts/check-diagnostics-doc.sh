#!/usr/bin/env bash
# Fail if docs/diagnostics.md has drifted from the registry
# (compiler/keelc-diag/src/registry.rs). Uses rustc directly so it adds no
# project dependency.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

mkdir -p target/harness
rustc --edition=2021 -D warnings scripts/gen-diagnostics-doc.rs -o target/harness/gen-diagnostics-doc
target/harness/gen-diagnostics-doc --check
