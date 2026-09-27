# Glyph v0.1 MVP

## Hypothesis under test

An AI agent can edit sophisticated visual work more reliably when it operates on stable semantic identities and relationships instead of rewriting raw SVG/path/UI code or regenerating the whole image.

## Implemented acceptance evidence

- `.glyph` → GSIR compilation works.
- Stable node IDs and semantic role queries work.
- Authored and derived geometry are separate.
- Provenance records source file + line.
- Relational constraints re-solve geometry.
- Revision-safe transactions reject stale edits.
- Semantic diff reports field-level changes by node.
- Canvas width change re-centers constrained title/mascot in tests.
- SVG retains stable IDs and semantic role metadata.
- CommanderDraw bridge renders PNG and fail-closed PDF output.
- Renderer conformance reports exact/degraded/unsupported backend behavior; target strategy separates visual exports from semantic codegen.
- Direct manipulation edits authored positions when free, translates paths through affine source transforms without rewriting path geometry, co-edits compatible equality offsets, preserves inequalities as feasibility guards, and inverts unambiguous typed component bindings into instance parameters. Rect/image/ellipse/text-frame resize is semantic; explicit edge/corner handles preserve opposite anchors; structural groups translate member leaves; unrotated paths preserve `d` while supporting compatible translate relations and explicit component affine bindings; unsafe/unsupported intent fails closed.
- Studio undo/redo restores validated source snapshots; free/relation/path/component drag and rect/image/ellipse/path semantic resize round-trip through history, while infeasible constraint guards and unsupported inverse cases leave source unchanged.
- Benchmark patch modifies two fields on `tree_right`, with 0 unrelated authored nodes changed and 0 required violations.

## Test suite

`cargo test` currently has 110 passing tests covering parsing/solving, dimensional/schema safety, components/imports, surgical property and constraint source editing, candidate-validated multi-constraint drag/resize intent, semantic text-frame + multi-edge resize, structural group membership/translation, affine path manipulation and compatible constrained/component inverse cases, accessibility semantics/auditing, display-list lowering, SVG/CommanderDraw semantics, CoreText typography, variable fonts, Vision reconstruction contracts, renderer conformance, target strategy, PDF fail-closed behavior, and responsive canvas re-solving. Studio has 7 integration tests covering semantic drag/resize, multi-edge handles, structural group drag, path drag/resize and undo, component inverse edits, constraint editing/guard resolution, text-frame resize, and source-snapshot undo/redo.

## Current limitations after post-gate expansion

- Visual quality is not implied by compiler correctness or benchmark success.
- Bounded Image→Glyph exists for structured UI/text/shape reconstruction; arbitrary photo/illustration reconstruction is not production-ready.
- Native Studio v0.1 now has constraint-aware positional drag, compatible multi-equality offset co-editing with inequality feasibility guards, text-frame resize, explicit multi-edge/corner handles, structural group translation, affine path drag/resize, compatible translate-relation inversion, explicit component-owned affine path parameter inversion, first-class constraint badges plus relation/strength/offset/remove/add editing, and validated source-snapshot undo/redo. Rotated-path resize and ambiguous mixed component/constraint ownership still fail closed.
- SwiftUI/Web Canvas semantic codegen is strategy-only and reports unsupported until explicit subset emitters and round-trip contracts exist.
- PDF is a visual export through CommanderDraw; Glyph semantic IDs are not promised as a PDF round-trip format.
- The Cassowary-class solver is rebuilt for each `solve_scene` call; a long-lived incremental editing session API is not exposed yet.
- On macOS, typography now uses CoreText shaping, deterministic line breaking, real ascent/descent/leading metrics, glyph counts, explicit font resolution/substitution/fallback diagnostics, letter spacing, ligature policy, and validated OpenType variable-font axes. Non-macOS builds retain a deterministic heuristic fallback until a cross-platform shaper is added; CommanderDraw currently preserves advanced typography metadata but does not guarantee exact CoreText feature/axis rendering.
- Motion is declarative sampled motion v1, not a full timeline/physics runtime.

## Formal StepIsland comparison

The core gate passed. The remaining cross-representation benchmark compares a representative StepIsland scene against raw SVG, SwiftUI and current CommanderDraw on:

- number of literal coordinates;
- edit locality;
- unintended authored changes;
- aspect-ratio adaptation;
- agent tool calls / failed edits;
- render reproducibility;
- semantic inspectability.

If Glyph does not materially reduce collateral edits and improve responsive/semantic manipulation, the abstraction should be reconsidered before Vision or Studio receives major investment.
