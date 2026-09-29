#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/glyph"
HELPER="$ROOT/studio/.build/release/GlyphVision"
IMAGE="$ROOT/benchmarks/generated/core-gate/15_ui_screen.png"
OUT="$ROOT/benchmarks/generated/vision-gate"
mkdir -p "$OUT"
cd "$ROOT/studio"
swift build -c release --product GlyphVision
cd "$ROOT"
cargo build --release
"$BIN" import-image "$IMAGE" -o "$OUT/a.glyph" --vision-helper "$HELPER" --manifest-output "$OUT/a.manifest.json"
"$BIN" import-image "$IMAGE" -o "$OUT/b.glyph" --vision-helper "$HELPER" --manifest-output "$OUT/b.manifest.json"
cmp "$OUT/a.glyph" "$OUT/b.glyph"
cmp "$OUT/a.manifest.json" "$OUT/b.manifest.json"
"$BIN" check "$OUT/a.glyph"
"$BIN" render "$OUT/a.glyph" --output "$OUT/a.svg"
grep -q '<svg' "$OUT/a.svg"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); o=d["observations"]; assert len(o)==9; assert sum(x["kind"]=="ellipse" for x in o)==3; assert sum(x["kind"]=="text" for x in o)==2' "$OUT/a.manifest.json"
[[ "$(grep -c '^  ellipse ' "$OUT/a.glyph")" -eq 3 ]]
[[ "$(grep -c '^  text ' "$OUT/a.glyph")" -eq 2 ]]
! grep -q 'max_width' "$OUT/a.glyph"
grep -A2 'content "Check in";' "$OUT/a.glyph" | grep -q 'align "center"'
echo "Vision gate passed: deterministic import, 9 nodes, 3 ellipses, centered CTA text, no OCR wrapping constraint."
