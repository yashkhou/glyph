#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/glyph"
cd "$ROOT"
cargo test
cargo build --release
mkdir -p exports
"$BIN" check examples/sakura_lake.glyph
"$BIN" compile examples/sakura_lake.glyph --output exports/sakura_lake.gsir.json
"$BIN" apply exports/sakura_lake.gsir.json 1 '[{"op":"set","node":"tree_right","path":"props.ry","value":305},{"op":"set","node":"tree_right","path":"props.rx","value":142}]' --output exports/sakura_lake.verify-patched.gsir.json
"$BIN" diff exports/sakura_lake.gsir.json exports/sakura_lake.verify-patched.gsir.json
"$BIN" render exports/sakura_lake.verify-patched.gsir.json --output exports/sakura_lake.verify-patched.svg
"$BIN" render-png exports/sakura_lake.verify-patched.gsir.json --output exports/sakura_lake.verify-patched.png
"$BIN" apply exports/sakura_lake.gsir.json 1 '[{"op":"set_canvas","width":1600}]' --output exports/sakura_lake.wide.gsir.json
"$BIN" why exports/sakura_lake.wide.gsir.json title
