# Shared Agent Layer

`.agents/` is the canonical home for agent-facing settings and slash-command
workflows. `.claude` is a symlink to this directory so Claude Code reads the
same files as other agent surfaces.

Do not copy files between `.agents/` and `.claude/`. Update `.agents/`, keep the
symlink intact, and run `scripts/check-harness.sh`.
