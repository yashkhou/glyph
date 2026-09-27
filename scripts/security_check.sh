#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[security] formatting + robustness corpus"
cargo fmt --check
cargo test --test robustness

echo "[security] project containment/hash regressions"
cargo test project_assets_fail_closed_on_hash_mismatch_and_root_escape
cargo test project_import_rejects_root_escape

echo "[security] dependency audit"
if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit
elif cargo audit --version >/dev/null 2>&1; then
  cargo audit
else
  echo "cargo-audit is not installed; dependency advisory scan skipped (deterministic security gates still passed)"
fi

echo "[security] PASS"
