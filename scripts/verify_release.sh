#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "[release] source gates"
cargo fmt --check
cargo test
./scripts/verify.sh

echo "[release] hardening gates"
./scripts/security_check.sh
./scripts/profile_core.sh

echo "[release] packaging"
ARCHIVE="$(./scripts/package_release.sh | tail -1)"
[[ -s "$ARCHIVE" ]]
[[ -s "$ARCHIVE.sha256" ]]
tar -tzf "$ARCHIVE" | grep -q '/bin/glyph$'
tar -tzf "$ARCHIVE" | grep -q '/bin/GlyphStudio$'
tar -tzf "$ARCHIVE" | grep -q '/bin/GlyphVision$'
echo "[release] PASS: $ARCHIVE"
