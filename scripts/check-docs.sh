#!/usr/bin/env bash
# Validate local Markdown links and keep public documentation reachable from
# README.md. The checker uses rustc directly so it adds no project dependency.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

mkdir -p target/harness
rustc --edition=2021 -D warnings scripts/check-docs.rs -o target/harness/check-docs
target/harness/check-docs
