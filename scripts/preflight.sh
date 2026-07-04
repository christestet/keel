#!/usr/bin/env bash
# The executable definition of done. Runs the full local gate; CI uses the same
# commands but scopes expensive jobs to the files changed in a PR/push.
# Humans and agents alike: run this before declaring any task complete
# (root AGENTS.md, "What done means").
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

step() { echo; echo "==> $*"; }

step "agent-harness self-check"
scripts/check-harness.sh

step "documentation graph"
scripts/check-docs.sh

step "diagnostics doc sync"
scripts/check-diagnostics-doc.sh

step "lsp transcript fixtures"
scripts/check-lsp-fixtures.sh

step "cargo fmt --all --check"
cargo fmt --all --check

step "cargo clippy --workspace --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

step "cargo test --workspace"
cargo test --workspace

step "conformance suite: structure"
cargo run -p conformance-runner -- --check

# Full conformance execution exists once keelc-driver joins the workspace (M3+).
# The runner's built-in default is M1; "done" means the *current* roadmap
# milestone, so default it here. Bump on every milestone exit (ROADMAP.md);
# export KEEL_MILESTONE to validate a different gate.
export KEEL_MILESTONE="${KEEL_MILESTONE:-M8}"
if cargo metadata --no-deps --format-version 1 | grep -q '"name":"keelc-driver"'; then
  step "build keelc"
  cargo build --release -p keelc-driver
  step "conformance suite: full run"
  export GOCACHE="$PWD/target/gocache"
  cargo run -p conformance-runner -- --keelc target/release/keelc \
    ${KEEL_MILESTONE:+--milestone "$KEEL_MILESTONE"}
else
  echo "keelc-driver not in workspace yet (pre-M3) — structure check only"
fi

echo
echo "preflight: green"
