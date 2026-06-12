#!/usr/bin/env bash
# Agent-harness self-check: keeps the AGENTS.md/CLAUDE.md guidance layers, the
# preflight entry point, and the .agents/.claude config from drifting apart as the
# repo grows. CI runs this on every commit; scripts/preflight.sh runs it locally.
#
# Growing the harness (see root AGENTS.md, "The agent harness"): when a new
# area gets its own AGENTS.md, register the directory in REQUIRED below.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

fail=0
err() { echo "harness: FAIL $1" >&2; fail=1; }

# 1. Areas that must carry agent guidance.
REQUIRED=(
  .
  compiler
  tests/conformance
  docs/spec
  docs/kdr
  examples
)
for d in "${REQUIRED[@]}"; do
  [ -f "$d/AGENTS.md" ] || err "$d/AGENTS.md missing"
done

# 2. Every AGENTS.md, anywhere, must have a sibling CLAUDE.md symlink so that
#    Claude Code and AGENTS.md-native agents read identical guidance.
while IFS= read -r agents; do
  claude="$(dirname "$agents")/CLAUDE.md"
  if [ ! -L "$claude" ]; then
    err "$claude must be a symlink to AGENTS.md (one source of truth per directory)"
  elif [ "$(readlink "$claude")" != "AGENTS.md" ]; then
    err "$claude points to $(readlink "$claude"), expected AGENTS.md"
  fi
done < <(find . -path ./target -prune -o -name AGENTS.md -print | sort)

# 3. Entry points the guidance tells agents to run must exist and be executable.
for s in scripts/preflight.sh scripts/check-harness.sh; do
  [ -x "$s" ] || err "$s missing or not executable"
done

# 4. Shared agent layer: .agents is canonical; .claude is only a compatibility
#    symlink so all agent surfaces read identical config and commands.
[ -d .agents ] || err ".agents directory missing"
if [ ! -L .claude ]; then
  err ".claude must be a symlink to .agents"
elif [ "$(readlink .claude)" != ".agents" ]; then
  err ".claude points to $(readlink .claude), expected .agents"
fi
[ -f .agents/settings.json ] || err ".agents/settings.json missing"
for c in preflight new-case new-kdr wiki-note; do
  [ -f ".agents/commands/$c.md" ] || err ".agents/commands/$c.md missing"
  [ -f ".claude/commands/$c.md" ] || err ".claude/commands/$c.md missing via .claude symlink"
done

if [ "$fail" -ne 0 ]; then
  echo "harness: see 'The agent harness' in the root AGENTS.md for how these pieces fit" >&2
  exit 1
fi
echo "harness: ok"
