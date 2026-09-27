# Semantic Code Generation

Glyph separates **visual export** from **semantic code generation**. SVG, PNG and PDF are visual deliverables. SwiftUI and Web Canvas output are editable source targets with a deliberately smaller contract.

The v1 codegen contract is **fail closed**: Glyph emits native source only when every rendered item fits the documented subset. Unsupported semantics return item-level reasons instead of silently rasterizing, flattening, or guessing.

## Commands

```bash
glyph export-swiftui scene.glyph --output GlyphSceneView.swift
glyph export-web-canvas scene.glyph --output scene.mjs
glyph targets scene.glyph
```

`glyph targets` reports scene-specific `exact`, `degraded`, or `unsupported` status for each codegen target.

## Supported v1 subset

Both emitters currently accept:

- canvas size and solid hexadecimal background colors;
- rectangles, rounded rectangles and ellipses;
- non-rendering structural groups;
- basic text using resolved Glyph line breaks and frame geometry;
- solid hexadecimal fill/stroke colors and opacity;
- stable Glyph object identity.

SwiftUI additionally maps stable IDs to `.accessibilityIdentifier` and supports bounded label, hint, hidden, reading-order, image-trait and header-trait metadata.

Web Canvas exports the stable object map as `glyphObjects`. Canvas has no native per-object accessibility tree, so supported accessibility metadata remains inspectable data rather than being misrepresented as DOM semantics.

## Degraded semantics

Text codegen is currently classified as `degraded`, not `exact`. Glyph preserves its resolved line breaks, frame, font request, size and alignment, but the destination runtime may shape glyphs differently from the macOS CoreText metrics that produced the Glyph scene.

Web Canvas also refuses non-zero letter spacing until portable runtime behavior is strong enough for the contract.

## Unsupported in v1

Codegen rejects a scene when it contains semantics that cannot yet be represented readably and reversibly enough:

- arbitrary path geometry;
- images whose resource ownership/bundling is not explicit;
- affine transforms;
- clips or masks and their definition-only nodes;
- typed gradients/paints;
- explicit stroke line-cap/join semantics;
- variable-font axes or non-standard ligature policy;
- text strokes;
- interactive accessibility without an authored action contract;
- label-reference/live-region relationships requiring application wiring.

For these scenes, use Glyph's visual export path or restructure the semantic source. The emitter never falls back to embedding an SVG/raster payload while calling the result semantic native code.

## Identity and editability

The generated files are deliverable source code, but Glyph remains canonical. Stable IDs make generated objects traceable back to the source scene; editing generated code does not automatically round-trip into `.glyph`.

The intended workflow is:

```text
.glyph / GSIR → display list → subset assessment → native source
```

not:

```text
arbitrary pixels → guessed native UI code → pretend round trip
```

The subset can expand only when a new feature has an explicit deterministic lowering and a truthful conformance classification.
