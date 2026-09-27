#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/glyph"
cd "$ROOT"
cargo test
cargo build --release
mkdir -p exports
TMP_VERIFY="$(mktemp -d /tmp/glyph-verify.XXXXXX)"
trap 'rm -rf "$TMP_VERIFY"' EXIT
cat > "$TMP_VERIFY/drag.glyph" <<'EOF'
scene "Drag Verify" {
  canvas 320 220;
  rect free @id("free") { x 20px; y 30px; width 80px; height 60px; fill "#336699"; }
  rect related @id("related") { x 180px; y 120px; width 70px; height 50px; fill "#995544"; }
  rect ambiguous @id("ambiguous") { x 40px; y 140px; width 60px; height 40px; fill "#557755"; }
  constrain related.center_x == canvas.center_x @required;
  constrain ambiguous.center_x == canvas.center_x @required;
  constrain ambiguous.left >= 0 @strong;
}
EOF
"$BIN" drag-plan "$TMP_VERIFY/drag.glyph" free 12 -7 > "$TMP_VERIFY/free-plan.json"
"$BIN" source-drag "$TMP_VERIFY/drag.glyph" free 12 -7 --output "$TMP_VERIFY/moved.glyph"
"$BIN" check "$TMP_VERIFY/moved.glyph"
grep -q 'x 32px;' "$TMP_VERIFY/moved.glyph"
grep -q 'y 23px;' "$TMP_VERIFY/moved.glyph"
"$BIN" resize-plan "$TMP_VERIFY/drag.glyph" free 15 -5 > "$TMP_VERIFY/free-resize-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/free-resize-plan.json"
"$BIN" source-resize "$TMP_VERIFY/drag.glyph" free 15 -5 --output "$TMP_VERIFY/resized.glyph"
"$BIN" check "$TMP_VERIFY/resized.glyph"
grep -q 'width 95px;' "$TMP_VERIFY/resized.glyph"
grep -q 'height 55px;' "$TMP_VERIFY/resized.glyph"
"$BIN" drag-plan "$TMP_VERIFY/drag.glyph" related 12 0 > "$TMP_VERIFY/related-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/related-plan.json"
grep -q '"constraint_id": "constraint.1"' "$TMP_VERIFY/related-plan.json"
"$BIN" source-drag "$TMP_VERIFY/drag.glyph" related 12 0 --output "$TMP_VERIFY/related.glyph"
"$BIN" check "$TMP_VERIFY/related.glyph"
grep -q 'constrain related.center_x == canvas.center_x + 12 @required;' "$TMP_VERIFY/related.glyph"
"$BIN" drag-plan "$TMP_VERIFY/drag.glyph" ambiguous 12 0 > "$TMP_VERIFY/ambiguous-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/ambiguous-plan.json"
grep -q '"constraint_id": "constraint.2"' "$TMP_VERIFY/ambiguous-plan.json"
"$BIN" source-drag "$TMP_VERIFY/drag.glyph" ambiguous 12 0 --output "$TMP_VERIFY/ambiguous-moved.glyph"
"$BIN" check "$TMP_VERIFY/ambiguous-moved.glyph"
grep -q 'constrain ambiguous.center_x == canvas.center_x + 12 @required;' "$TMP_VERIFY/ambiguous-moved.glyph"
grep -q 'constrain ambiguous.left >= 0 @strong;' "$TMP_VERIFY/ambiguous-moved.glyph"
"$BIN" drag-plan "$TMP_VERIFY/drag.glyph" ambiguous -200 0 > "$TMP_VERIFY/ambiguous-blocked-plan.json"
grep -q '"allowed": false' "$TMP_VERIFY/ambiguous-blocked-plan.json"
grep -q '"constraint.3"' "$TMP_VERIFY/ambiguous-blocked-plan.json"
"$BIN" constraints "$TMP_VERIFY/drag.glyph" --node ambiguous > "$TMP_VERIFY/ambiguous-constraints.json"
grep -q '"id": "constraint.2"' "$TMP_VERIFY/ambiguous-constraints.json"
grep -q '"id": "constraint.3"' "$TMP_VERIFY/ambiguous-constraints.json"
"$BIN" constraint-edit "$TMP_VERIFY/drag.glyph" 2 --strength strong --offset 5px --output "$TMP_VERIFY/constraint-edited.glyph"
grep -q 'constrain ambiguous.center_x == canvas.center_x + 5px @strong;' "$TMP_VERIFY/constraint-edited.glyph"
"$BIN" constraint-remove "$TMP_VERIFY/drag.glyph" 3 --output "$TMP_VERIFY/constraint-removed.glyph"
"$BIN" drag-plan "$TMP_VERIFY/constraint-removed.glyph" ambiguous 12 0 > "$TMP_VERIFY/constraint-resolved-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/constraint-resolved-plan.json"
"$BIN" constraint-add "$TMP_VERIFY/constraint-removed.glyph" ambiguous.top ge 0 --strength medium --output "$TMP_VERIFY/constraint-added.glyph"
grep -q 'constrain ambiguous.top >= 0 @medium;' "$TMP_VERIFY/constraint-added.glyph"
"$BIN" check "$TMP_VERIFY/constraint-added.glyph"
cat > "$TMP_VERIFY/component.glyph" <<'EOF'
scene "Component Verify" {
  canvas 500 300;
  component card {
    param px: Length -> bg.x = 10px;
    param w: Length -> bg.width = 100px;
    rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
  }
  instance card card1 @id("card1") { x 100px; scale 2; param px 15px; param w 120px; }
}
EOF
"$BIN" source-drag "$TMP_VERIFY/component.glyph" card1/bg 20 0 --output "$TMP_VERIFY/component-moved.glyph"
grep -q 'param px 25px;' "$TMP_VERIFY/component-moved.glyph"
"$BIN" source-resize "$TMP_VERIFY/component-moved.glyph" card1/bg 20 0 --output "$TMP_VERIFY/component-resized.glyph"
grep -q 'param w 130px;' "$TMP_VERIFY/component-resized.glyph"
"$BIN" check "$TMP_VERIFY/component-resized.glyph"
cat > "$TMP_VERIFY/component-extract.glyph" <<'EOF'
scene "Component Extract Verify" {
  canvas 420 240;
  rect back @id("back") { x 10; y 10; width 100; height 80; }
  rect card @id("card") { x 120; y 30; width 100; height 60; }
  text label @id("label") { content "Card"; x 132; y 64; size 18px; }
  rect front @id("front") { x 240; y 20; width 100; height 80; }
}
EOF
"$BIN" component-extract "$TMP_VERIFY/component-extract.glyph" card_component card1 card label --output "$TMP_VERIFY/component-extracted.glyph" > "$TMP_VERIFY/component-extract.json"
grep -q '"instance": "card1"' "$TMP_VERIFY/component-extract.json"
grep -q '^  component card_component {' "$TMP_VERIFY/component-extracted.glyph"
grep -q '^    rect card' "$TMP_VERIFY/component-extracted.glyph"
grep -q '^  instance card_component card1 { }' "$TMP_VERIFY/component-extracted.glyph"
"$BIN" check "$TMP_VERIFY/component-extracted.glyph"
"$BIN" components "$TMP_VERIFY/component-extracted.glyph" > "$TMP_VERIFY/component-extracted-catalog.json"
grep -q '"name": "card_component"' "$TMP_VERIFY/component-extracted-catalog.json"
cat > "$TMP_VERIFY/hierarchy.glyph" <<'EOF'
scene "Hierarchy Verify" {
  canvas 420 240;
  component badge { rect bg @id("bg") { x 0; y 0; width 70; height 30; } }
  rect back @id("back") { x 10; y 10; width 100; height 80; }
  instance badge badge1 @id("badge1") { x 120; y 30; }
  rect front @id("front") { x 220; y 20; width 100; height 80; }
}
EOF
"$BIN" hierarchy "$TMP_VERIFY/hierarchy.glyph" > "$TMP_VERIFY/hierarchy.json"
grep -q '"id": "badge1"' "$TMP_VERIFY/hierarchy.json"
grep -q '"kind": "instance:badge"' "$TMP_VERIFY/hierarchy.json"
"$BIN" node-move "$TMP_VERIFY/hierarchy.glyph" badge1/bg back before --output "$TMP_VERIFY/hierarchy-moved.glyph"
"$BIN" check "$TMP_VERIFY/hierarchy-moved.glyph"
[[ "$(grep -n 'instance badge badge1' "$TMP_VERIFY/hierarchy-moved.glyph" | cut -d: -f1)" -lt "$(grep -n 'rect back' "$TMP_VERIFY/hierarchy-moved.glyph" | cut -d: -f1)" ]]
cat > "$TMP_VERIFY/path.glyph" <<'EOF'
scene "Path Verify" {
  canvas 300 200;
  path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; }
}
EOF
"$BIN" drag-plan "$TMP_VERIFY/path.glyph" mark 15 -5 > "$TMP_VERIFY/path-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/path-plan.json"
"$BIN" source-drag "$TMP_VERIFY/path.glyph" mark 15 -5 --output "$TMP_VERIFY/path-moved.glyph"
grep -q 'translate_x 15;' "$TMP_VERIFY/path-moved.glyph"
grep -q 'translate_y -5;' "$TMP_VERIFY/path-moved.glyph"
grep -q 'd "M10 20 L60 20 L60 70 Z";' "$TMP_VERIFY/path-moved.glyph"
"$BIN" check "$TMP_VERIFY/path-moved.glyph"
"$BIN" object-map "$TMP_VERIFY/path-moved.glyph" > "$TMP_VERIFY/path-map.json"
grep -q '25.0' "$TMP_VERIFY/path-map.json"
"$BIN" resize-plan "$TMP_VERIFY/path-moved.glyph" mark 25 10 > "$TMP_VERIFY/path-resize-plan.json"
grep -q '"allowed": true' "$TMP_VERIFY/path-resize-plan.json"
"$BIN" source-resize "$TMP_VERIFY/path-moved.glyph" mark 25 10 --output "$TMP_VERIFY/path-resized.glyph"
grep -q 'scale_x 1.5;' "$TMP_VERIFY/path-resized.glyph"
grep -q 'scale_y 1.2;' "$TMP_VERIFY/path-resized.glyph"
grep -q 'd "M10 20 L60 20 L60 70 Z";' "$TMP_VERIFY/path-resized.glyph"
"$BIN" check "$TMP_VERIFY/path-resized.glyph"
"$BIN" object-map "$TMP_VERIFY/path-resized.glyph" > "$TMP_VERIFY/path-resized-map.json"
grep -q '75.0' "$TMP_VERIFY/path-resized-map.json"
grep -q '60.0' "$TMP_VERIFY/path-resized-map.json"
"$BIN" check examples/sakura_lake.glyph
"$BIN" compile examples/sakura_lake.glyph --output exports/sakura_lake.gsir.json
"$BIN" apply exports/sakura_lake.gsir.json 1 '[{"op":"set","node":"tree_right","path":"props.ry","value":305},{"op":"set","node":"tree_right","path":"props.rx","value":142}]' --output exports/sakura_lake.verify-patched.gsir.json
"$BIN" diff exports/sakura_lake.gsir.json exports/sakura_lake.verify-patched.gsir.json
"$BIN" render exports/sakura_lake.verify-patched.gsir.json --output exports/sakura_lake.verify-patched.svg
"$BIN" render-png exports/sakura_lake.verify-patched.gsir.json --output exports/sakura_lake.verify-patched.png
"$BIN" conformance exports/sakura_lake.verify-patched.gsir.json
"$BIN" targets exports/sakura_lake.verify-patched.gsir.json
"$BIN" export-pdf exports/sakura_lake.verify-patched.gsir.json --output exports/sakura_lake.verify-patched.pdf
[[ "$(head -c 5 exports/sakura_lake.verify-patched.pdf)" == "%PDF-" ]]
cat > "$TMP_VERIFY/codegen.glyph" <<'EOF'
scene "Codegen Verify" {
  canvas 320 200 background "#f8f8f8";
  rect card @id("card") { x 20; y 30; width 180; height 90; rx 12; fill "#336699"; a11y_label "Card"; }
  text title @id("title") { content "Glyph codegen"; x 24; y 160; size 22; max_width 220; fill "#111111"; }
}
EOF
"$BIN" export-swiftui "$TMP_VERIFY/codegen.glyph" --output "$TMP_VERIFY/GlyphSceneView.swift"
"$BIN" export-web-canvas "$TMP_VERIFY/codegen.glyph" --output "$TMP_VERIFY/glyph-scene.mjs"
grep -q 'accessibilityIdentifier("card")' "$TMP_VERIFY/GlyphSceneView.swift"
grep -q 'export const glyphObjects' "$TMP_VERIFY/glyph-scene.mjs"
xcrun swiftc -typecheck "$TMP_VERIFY/GlyphSceneView.swift"
NODE_BIN="${GLYPH_NODE_BIN:-${NODE_BINARY:-$(command -v node || true)}}"
if [[ -z "$NODE_BIN" ]]; then
  for candidate in /opt/homebrew/bin/node /usr/local/bin/node /usr/bin/node; do
    [[ -x "$candidate" ]] && NODE_BIN="$candidate" && break
  done
fi
[[ -n "$NODE_BIN" && -x "$NODE_BIN" ]]
"$NODE_BIN" --check "$TMP_VERIFY/glyph-scene.mjs"
"$BIN" apply exports/sakura_lake.gsir.json 1 '[{"op":"set_canvas","width":1600}]' --output exports/sakura_lake.wide.gsir.json
"$BIN" why exports/sakura_lake.wide.gsir.json title
(cd "$ROOT/studio" && GLYPH_BIN="$BIN" swift test)
