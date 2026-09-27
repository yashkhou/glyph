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

`cargo test` currently has **137 passing Rust tests total** (132 core integration tests, 2 deterministic robustness/mutation tests, and 3 LSP unit tests) covering parsing/solving, dimensional/schema safety, components/imports, surgical property and constraint source editing, candidate-validated multi-constraint drag/resize intent, semantic text-frame + multi-edge resize, structural group membership/translation plus source-authoritative create/add/remove/ungroup and top-level render-order reordering, affine path manipulation and compatible constrained/component inverse cases, accessibility semantics/auditing, production-linked language diagnostics/completion/navigation/refactors/quick fixes, transitive project-aware component/style/paint definition/references/cross-file rename with open-document overlays, content-addressed project assets and deterministic self-contained bundles, UTF-16-safe incremental LSP synchronization and real stdio LSP framing, richer sampled motion, hybrid Vision import, display-list lowering, SVG/CommanderDraw semantics, CoreText typography, variable fonts, renderer conformance, target strategy, PDF fail-closed behavior, and responsive canvas re-solving. Studio has **10 integration tests** covering semantic drag/resize, multi-edge handles, structural group drag, multi-select grouping/ungrouping, direct source/render-order reordering with undo, component-instance-owner reordering from selected children, path drag/resize and undo, component inverse edits, constraint editing/guard resolution, text-frame resize, and source-snapshot undo/redo.

## Current limitations after post-gate expansion

- Visual quality is not implied by compiler correctness or benchmark success.
- Image→Glyph now supports structured reconstruction plus deterministic `auto`/`hybrid` fallback: ambiguous photo/illustration inputs preserve the original raster and attach confidence-bearing non-rendering semantic overlays. Arbitrary photo-to-clean-vector reconstruction remains intentionally unsupported.
- Native Studio v0.1 now has constraint-aware positional drag, compatible multi-equality offset co-editing with inequality feasibility guards, text-frame resize, explicit multi-edge/corner handles, structural group translation plus multi-select grouping/ungrouping, and source-authoritative one-step render-order reordering for directly-authored top-level nodes and whole component-instance owners (including when a child is selected). It also supports affine path drag/resize, compatible translate-relation inversion, explicit component-owned affine path parameter inversion, first-class constraint badges plus relation/strength/offset/remove/add editing, and validated source-snapshot undo/redo. Rotated-path resize and ambiguous mixed component/constraint ownership still fail closed.
- SwiftUI/Web Canvas semantic codegen now has explicit bounded emitters over the display list. Rect/ellipse/group/basic-text scenes with simple solid styling emit editable native source with stable identity; CoreText-to-target text is classified as degraded, while paths, images, affine transforms, clips/masks, gradients, advanced typography and interaction contracts fail closed. The exact subset is documented in `docs/CODEGEN.md`.
- PDF is a visual export through CommanderDraw; Glyph semantic IDs are not promised as a PDF round-trip format.
- The Cassowary-class solver is rebuilt for each `solve_scene` call; a long-lived incremental editing session API is not exposed yet.
- Language tooling now has deterministic diagnostics, context-aware completion, document-scoped node definition/references/rename, canonical hover + semantic tokens, project-aware component/style/paint definition/references/cross-file rename and imported-symbol hover across transitive import namespaces, accessibility quick fixes, and a real stdio LSP with UTF-16-safe incremental synchronization plus open-document overlays. Richer code actions and deeper editor refactoring workflows remain future tooling work.
- On macOS, typography now uses CoreText shaping, deterministic line breaking, real ascent/descent/leading metrics, glyph counts, explicit font resolution/substitution/fallback diagnostics, letter spacing, ligature policy, and validated OpenType variable-font axes. Non-macOS builds retain a deterministic heuristic fallback until a cross-platform shaper is added; CommanderDraw currently preserves advanced typography metadata but does not guarantee exact CoreText feature/axis rendering.
- Motion is now a deterministic sampled timeline with typed `s`/`ms` timing, delay, finite repeat, alternate playback and deterministic ease/spring presets. It deliberately remains a sampled semantic runtime rather than a continuous physics simulation engine.
- Project hardening now verifies manifest entry identity, transitive imported source hashes, SHA-256-addressed assets and path containment, then publishes self-contained dependency-closure bundles through a staged re-validation step. A public registry/package ecosystem is still intentionally out of scope.
- Release hardening includes a deterministic parser mutation corpus, fail-closed malformed source-edit tests, a 5,000-node release-mode performance budget, containment/hash security regressions, full Studio verification and checksummed macOS archives. `cargo-audit 0.22.2` is installed in the current development environment and the RustSec scan passes against the current lockfile.

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
