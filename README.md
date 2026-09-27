# Glyph

Glyph is a programmable design format and agent-native semantic visual runtime. It keeps **design identity and intent** above raw SVG/path geometry so humans and AI can query, patch, re-solve, diff, and render scenes without rewriting the entire asset.

## Status

`v0.1.0` is a working research prototype. The canonical representation is **GSIR (Glyph Scene IR)**. The current resolver is Cassowary-class weighted linear solving with explicit required/soft constraint diagnostics; solves currently rebuild solver state per invocation rather than keeping a long-lived incremental editing session.

```text
.glyph source
    ↓
parser + typed scene model
    ↓
GSIR: stable IDs · semantic roles · authored state · provenance · constraints
    ↓
constraint resolution
    ↓
resolved geometry / backend-neutral display list
    ├── SVG
    ├── CommanderDraw .draw.json
    ├── CommanderDraw renderer → PNG
    └── CommanderDraw export → PDF
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
glyph export-pdf scene.v2.gsir.json --output scene.pdf
glyph conformance scene.v2.gsir.json
glyph accessibility scene.glyph
glyph diagnostics scene.glyph
glyph complete scene.glyph 12 4
glyph lsp
glyph targets scene.v2.gsir.json
glyph drag-plan scene.glyph card 12 -7
glyph source-drag scene.glyph card 12 -7 --output scene.moved.glyph
glyph resize-plan scene.glyph card 24 -10
glyph source-resize scene.glyph card 24 -10 --output scene.resized.glyph
glyph constraints scene.glyph --node card
glyph constraint-edit scene.glyph 2 --strength strong --offset 8px --output scene.constraint-edited.glyph
glyph constraint-remove scene.glyph 3 --output scene.constraint-removed.glyph
glyph constraint-add scene.glyph card.left ge 0 --strength required --output scene.constraint-added.glyph
```

## Commander integration

Glyph is registered as the local `glyph-design` Commander plugin for scene inspection/querying, revision-safe edits, semantic/visual diffs, runtime states/motion, source round-trip edits, first-class authored constraint operations, constraint-aware drag/resize planning, display-list/object-map inspection, renderer conformance, target strategy, bounded image import, and SVG/PNG/PDF/CommanderDraw export. Editor-facing language tooling is provided by the same core binary through deterministic `diagnostics`, context-aware `complete`, and stdio `lsp` commands.

The plugin executes the release binary at `target/release/glyph`. PNG and PDF deliberately reuse CommanderDraw rather than introducing another raster engine. Unsupported CommanderDraw semantics fail closed before those exports. Node resolves deterministically from `GLYPH_NODE_BIN` or `NODE_BINARY`, then process `PATH`, then standard Homebrew/system locations.

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
- the same display list lowers to CommanderDraw, PNG and fail-closed PDF output;
- renderer conformance and target reports distinguish exact/degraded/unsupported behavior instead of implying pixel equivalence;
- direct manipulation v1 edits free authored coordinates, translates paths through affine `translate_x`/`translate_y` without rewriting `d`, co-edits compatible equality offsets, preserves inequalities as feasibility guards, and inverts unambiguous typed component bindings back into instance `param` edits; semantic rect/image/ellipse/text-frame resize follows the same source-authoritative contract; Studio exposes explicit edge/corner handles, structural groups own validated membership and translate their leaf members semantically, and unrotated paths support source-authoritative affine resize including compatible translate relations and explicit component affine bindings; unsupported inverse intent fails closed;
- authored constraints are first-class source edit targets: relation, strength and offset can be changed surgically; constraints can be removed or added; semantic reparsing rejects dimensionally invalid edits; Studio surfaces constraint badges and inspector controls instead of forcing raw-text surgery;
- Studio semantic undo/redo restores validated source snapshots rather than canvas-only state;
- language tooling reuses the production parser/solver/accessibility contracts: diagnostics preserve parse locations, completions are context-aware, project files resolve local imports against unsaved entry overlays, and `glyph lsp` exposes full-document-sync diagnostics/completion over standard stdio JSON-RPC;
- CoreText typography, bounded Vision import, sampled motion, and the native Studio are present as scoped v1 implementations.

## Deliberately postponed

Full arbitrary-photo reconstruction, rotated-path inverse resize, mixed component-constraint path ownership, semantic SwiftUI/Web Canvas code generation, learned aesthetic constraints as authority, a public package ecosystem, and a huge semantic ontology remain out of scope. Accessibility semantics/auditing, text-frame resize, multi-edge handles, structural group membership/translation, compatible path-constraint resize, and explicit component-owned affine path inversion are now implemented where the source contract is unambiguous.

See `docs/ARCHITECTURE.md` and `docs/MVP.md`.
