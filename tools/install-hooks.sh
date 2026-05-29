#!/usr/bin/env bash
#
# Install the c2pa-text git hooks by pointing core.hooksPath at .githooks.
# Run once after cloning the standalone c2pa-text repository.
#
# Guard: this refuses to run unless the current git repository root IS the
# c2pa-text repo root. That prevents it from hijacking core.hooksPath when
# c2pa-text is vendored inside a larger monorepo that manages its own hooks.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GIT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ "${GIT_ROOT}" != "${ROOT}" ]; then
  echo "Refusing to install hooks: this does not look like a standalone c2pa-text checkout."
  echo "  git root : ${GIT_ROOT:-<none>}"
  echo "  expected : ${ROOT}"
  echo "If c2pa-text is vendored in a monorepo, configure hooks at the monorepo level instead."
  exit 1
fi

git config core.hooksPath .githooks
chmod +x .githooks/* tools/*.sh 2>/dev/null || true
echo "Installed git hooks: core.hooksPath=.githooks"
echo "pre-push will run scripts/test-all.sh before every push."
