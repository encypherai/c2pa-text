#!/usr/bin/env bash
#
# Run the full c2pa-text test suite across all four language implementations plus
# the cross-language golden parity / drift check. Run this locally before opening
# a pull request. It is also wired as a pre-push git hook
# (see scripts/install-hooks.sh).
#
# Each language suite loads golden/vectors.json and asserts byte-for-byte
# reproduction, so a green run proves the Rust/Python/TypeScript/Go SDKs produce
# identical embeddings against the stable golden state.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Make common toolchain locations discoverable in hook environments.
export PATH="$HOME/.cargo/bin:/usr/local/go/bin:$PATH"

status=0
run() {
  local name="$1"
  shift
  echo
  echo "=== ${name} ==="
  if "$@"; then
    echo "[${name}] OK"
  else
    echo "[${name}] FAILED"
    status=1
  fi
}

have() { command -v "$1" >/dev/null 2>&1; }

# --- Rust ---
if have cargo; then
  run "rust" bash -c "cd rust && cargo test --quiet"
else
  echo "=== rust ===" && echo "[rust] SKIPPED (cargo not found)"
fi

# --- Python ---
if have uv; then
  run "python" bash -c "cd python && uv run pytest -q -c pytest.ini"
elif have python3; then
  run "python" bash -c "cd python && PYTHONPATH=src python3 -m pytest -q"
else
  echo "=== python ===" && echo "[python] SKIPPED (python not found)"
fi

# --- TypeScript ---
if have npm; then
  run "typescript" bash -c "cd typescript && npm test --silent"
else
  echo "=== typescript ===" && echo "[typescript] SKIPPED (npm not found)"
fi

# --- Go ---
if have go; then
  run "go" bash -c "cd go && go vet ./... && go test ./..."
else
  echo "=== go ===" && echo "[go] SKIPPED (go not found)"
fi

# --- Golden parity / drift ---
echo
echo "=== golden parity (regenerate + drift) ==="
if have uv; then
  uv run --project python python golden/generate.py >/dev/null
elif have python3; then
  PYTHONPATH=python/src python3 golden/generate.py >/dev/null
fi
if git diff --quiet -- golden/ 2>/dev/null; then
  echo "[golden] OK (no drift)"
else
  echo "[golden] FAILED: golden/ changed after regeneration."
  echo "          Run 'python golden/generate.py' and commit the updated fixtures."
  git --no-pager diff --stat -- golden/ 2>/dev/null || true
  status=1
fi

echo
if [ "$status" -eq 0 ]; then
  echo "ALL CHECKS PASSED"
else
  echo "SOME CHECKS FAILED"
fi
exit "$status"
