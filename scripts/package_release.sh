#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
ARCH="$(uname -m)"
NAME="glyph-${VERSION}-macos-${ARCH}"
DIST="$ROOT/dist"
STAGE="$DIST/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/docs" "$STAGE/examples"

cargo build --release
(cd studio && swift build -c release)
cp target/release/glyph "$STAGE/bin/glyph"
cp studio/.build/release/GlyphStudio "$STAGE/bin/GlyphStudio"
cp studio/.build/release/GlyphVision "$STAGE/bin/GlyphVision"
cp README.md "$STAGE/README.md"
cp docs/ARCHITECTURE.md docs/MVP.md docs/SCHEMA_VERSIONING.md "$STAGE/docs/"
cp examples/sakura_lake.glyph "$STAGE/examples/"

(
  cd "$STAGE"
  shasum -a 256 bin/glyph bin/GlyphStudio bin/GlyphVision > SHA256SUMS
)
rm -f "$DIST/$NAME.tar.gz"
tar -C "$DIST" -czf "$DIST/$NAME.tar.gz" "$NAME"
shasum -a 256 "$DIST/$NAME.tar.gz" > "$DIST/$NAME.tar.gz.sha256"
echo "$DIST/$NAME.tar.gz"
