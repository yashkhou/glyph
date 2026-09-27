#!/bin/zsh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --release >/dev/null
BIN="$ROOT/target/release/glyph"
TMP="$(mktemp -d /tmp/glyph-profile.XXXXXX)"
trap 'rm -rf "$TMP"' EXIT
python3 - "$TMP/stress.glyph" <<'PY'
import sys
path = sys.argv[1]
count = 5000
with open(path, "w", encoding="utf-8") as f:
    f.write('scene "Profile" {\n  canvas 4096 4096;\n')
    for i in range(count):
        x = (i % 100) * 40
        y = (i // 100) * 40
        f.write(f'  rect n{i} @id("n{i}") {{ x {x}; y {y}; width 24; height 24; }}\n')
    f.write('}\n')
PY
python3 - "$BIN" "$TMP" <<'PY'
import json, subprocess, sys, time
bin_path, tmp = sys.argv[1:]
scene = f"{tmp}/stress.glyph"
limits = [("check", [bin_path, "check", scene], 5.0),
          ("compile", [bin_path, "compile", scene, "--output", f"{tmp}/stress.gsir.json"], 5.0)]
results = {}
for name, command, limit in limits:
    start = time.perf_counter()
    subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, check=True)
    elapsed = time.perf_counter() - start
    results[name] = round(elapsed, 4)
    if elapsed > limit:
        raise SystemExit(f"{name} exceeded {limit:.1f}s budget: {elapsed:.3f}s")
print(json.dumps({"nodes": 5000, "seconds": results, "status": "pass"}, sort_keys=True))
PY
