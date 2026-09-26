# Glyph

Glyph is a programmable design format and agent-native semantic visual runtime. It keeps **design identity and intent** above raw SVG/path geometry so humans and AI can query, patch, re-solve, diff, and render scenes without rewriting the entire asset.

## Status

`v0.1.0` is a working research prototype. The canonical representation is **GSIR (Glyph Scene IR)**. The current constraint engine is an intentionally small deterministic MVP resolver; a Cassowary-class incremental solver is a planned replacement behind the same IR/API.

```text
.glyph source
    ↓
parser + typed scene model
    ↓
GSIR: stable IDs · semantic roles · authored state · provenance · constraints
    ↓
constraint resolution
    ↓
resolved geometry
    ├── SVG
    ├── CommanderDraw .draw.json
    └── CommanderDraw renderer → PNG
```

## Why Glyph exists

SVG and CommanderDraw are good geometry/rendering layers. Glyph sits above them and preserves information they do not normally encode as first-class authoring state: semantic object identity, relational layout, authored-vs-derived values, source provenance, revision-safe semantic edits, and semantic diffs.

The key invariant for agent editing is simple:

> A surgical patch should change the requested authored object and report whether any unrelated authored object changed.

The included benchmark changes two authored properties on `tree_right`, reports **0 unrelated authored nodes changed**, **0 required constraints violated**, re-solves the scene, and renders the result.

## Example

```glyph
scene "Sakura Lake" @id("scene_sakura_lake") {
  canvas 1290 2796 background "#DDEFF1";

  ellipse lake @id("lake") {
    role "lake" "water" "midground";
    cx 645;
    cy 1660;
    rx 520;
    ry 270;
    fill "#72B6C8";
  }

  rect bridge @id("bridge") {
    role "bridge" "architecture";
    x 430;
    y 1590;
    width 430;
    height 56;
    fill "#C58C68";
  }

  constrain bridge.center_x == lake.center_x @strong;
  constrain bridge.center_y == lake.center_y - 70 @strong;
}
```

## CLI

```bash
glyph capabilities
glyph check scene.glyph
glyph compile scene.glyph --output scene.gsir.json
glyph query scene.gsir.json --role tree
glyph inspect scene.gsir.json --node tree_right
glyph why scene.gsir.json bridge
glyph apply scene.gsir.json 1 '[{"op":"set","node":"tree_right","path":"props.ry","value":305}]' --output scene.v2.gsir.json
glyph diff scene.gsir.json scene.v2.gsir.json
glyph render scene.v2.gsir.json --output scene.svg
glyph render-png scene.v2.gsir.json --output scene.png
```

## Commander integration

Glyph is registered as the local `glyph-design` Commander plugin with 11 tools:

`glyph_capabilities`, `glyph_check`, `glyph_compile`, `glyph_query`, `glyph_inspect`, `glyph_why`, `glyph_apply`, `glyph_diff`, `glyph_render_svg`, `glyph_render_png`, and `glyph_export_draw`.

The plugin executes the release binary at `target/release/glyph`. PNG rendering deliberately reuses CommanderDraw rather than introducing another raster engine.

## What v0.1 proves

- human-readable `.glyph` source parses into canonical GSIR;
- nodes have stable semantic IDs and free-form semantic roles;
- authored state is separate from derived/solved state;
- source provenance survives compilation;
- required/strong/medium/weak relational constraints are represented;
- scenes can be queried by semantic role;
- `why` exposes authored state, derived state, provenance, and constraints;
- transactions are revision-safe;
- semantic diffs operate by stable ID;
- canvas resizing re-solves centered objects without rewriting authored values;
- deterministic SVG preserves IDs/roles;
- CommanderDraw compatibility provides PNG output now.

## Deliberately postponed

Image→Glyph / photo reconstruction, a full visual editor, motion, learned aesthetic constraints, a public package system, and a huge semantic ontology are not in v0.1. They come after the core representation and agent-edit benchmarks justify them.

See `docs/ARCHITECTURE.md` and `docs/MVP.md`.
