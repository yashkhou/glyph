# Glyph — New Chat Handoff

## Paste this into the new chat

```text
@Commander Continue the Glyph project from the exact current repo state at /Users/joshuakhoury/Projects/Glyph.

First read, in this order:
1. docs/NEW_CHAT_HANDOFF.md
2. docs/GLYPH_RESEARCH_ARCHIVE.md
3. docs/CORE_GATE_REPORT.md
4. docs/ARCHITECTURE.md
5. docs/MVP.md

Treat NEW_CHAT_HANDOFF.md + GLYPH_RESEARCH_ARCHIVE.md + CORE_GATE_REPORT.md as the authoritative current context. Do not reset, clean, checkout, discard, or overwrite the existing working tree. It contains the active post-v0.1 implementation work from the previous chat.

After reading, inspect current git status and live Glyph capabilities, create a non-destructive snapshot/checkpoint, then continue the remaining roadmap in dependency order. Do not ask broad questions; fill routine implementation gaps yourself. Keep the research architecture intact: GSIR is canonical, .glyph is the human/agent layer, CommanderDraw is a rendering/export backend rather than Glyph's canonical model, and neural/vision features remain bounded and confidence-bearing rather than silently authoritative.
```

## Project identity

- Project: **Glyph**
- Repo: `/Users/joshuakhoury/Projects/Glyph`
- Commander workspace: `Glyph`
- Branch: `master`
- Last committed HEAD observed at handoff: `8d5ffccbb2308b0c1a100bb56bc0054f528b9925`
- Current product version: `0.1.0` research prototype, heavily expanded beyond the initial commit.
- Canonical representation: **GSIR — Glyph Scene IR**.
- Source extension: `.glyph`.

The working tree is intentionally dirty. It contains the implementation completed in this chat. **Do not use `git reset --hard`, clean untracked files, or restore the repo to HEAD.**
## Current implementation state

The architectural hypothesis has been proven enough to continue. Glyph is no longer just research.

Working today:

- `.glyph` source → GSIR compilation.
- Stable semantic IDs and open semantic roles.
- Authored values kept separate from solved/derived values.
- Source provenance and confidence metadata.
- Cassowary-class weighted linear solving with required/strong/medium/weak priorities.
- Deterministic tie-breaking and deep dependency-chain handling.
- Revision-safe atomic semantic transactions.
- Semantic diff and resolved visual diff by stable ID.
- Query-by-role, inspect and `why` diagnostics.
- Source round-trip property editing without flattening the whole file.
- Named styles, components/instances and seeded deterministic repeat.
- Assertions, named states and sampled motion v1.
- Backend-neutral display list + stable object map.
- SVG output and CommanderDraw `.draw.json`/PNG bridge.
- Native macOS Glyph Studio v0.1.
- Studio source rail, live resolved plate, semantic ledger and intent inspector.
- Direct rendered-plate click → stable Glyph ID → inspector selection.
- Studio inspector edits round-trip through `.glyph` source and refresh preview.
- Studio `LAST CHANGE` semantic visual-diff trace.
- Bounded Apple Vision image → editable Glyph import for structured UI screenshots.
- Vision import preserves inference provenance/confidence and remains explicitly non-authoritative.
- Commander plugin integration with **19 directly exposed Glyph tools** including `glyph_import_image`; the core CLI/capability contract also includes constraint query/editing and source-backed drag/resize planning.
- Dimensional units/types, source/GSIR versioning + migration, typed components/slots/variants/overrides, and local project imports are implemented.
- Canonical graphics now include image nodes, affine transforms, resolved clip/mask references, and typed linear/radial paints in display-list schema v3.
- Direct manipulation now candidate-validates compatible multi-constraint intent: equality offsets can co-edit, inequalities stay feasibility guards, and unsafe/newly-relaxed intent fails closed.
- Semantic resize supports rect/image/ellipse and unrotated authored paths; path resize uses affine scale/translation and preserves the authored SVG `d`. Studio exposes bottom-right handles for rect/image/ellipse/path.

Live `glyph capabilities` reports GSIR canonical, source schema v1 / GSIR schema v2, Cassowary-class weighted linear solver v1, display-list/object-map v3, typed paints/transforms/clips/masks, CoreText-backed text semantics, source-backed constraint/direct-manipulation APIs, Studio v0.1, sampled motion v1, visual diff v1, and bounded Apple Vision import v1.
## Verified evidence at handoff

- Rust integration suite: **92/92 passing**.
- Native Studio integration suite: **4/4 passing**.
- `cargo fmt --check`: passing.
- Release build: passing.
- Full `./scripts/verify.sh`: passing, including source-backed free/relation/component/path drag, rect/component/path resize, constraint editing, SVG/PNG/PDF export and Studio integration tests.
- Core effectiveness gate: **PASS**.
- Gate breadth: 15 artifact classes, 30 surgical edits, 0 unrelated authored mutations.
- Deep required-chain test: 51 nodes / 51 required constraints resolves fully.
- Responsive test: constrained objects re-solve without destructive authored rewrites.
- Determinism: repeated representative GSIR/SVG runs byte-identical; CommanderDraw PNG deterministic in measured environment.
- 5,000-node stress check restored to roughly 1.2–1.3 s after solver graph pruning.
- StepIsland cross-representation benchmark completed; a well-authored SwiftUI scene can match Glyph on a simple responsive case, while Glyph adds semantic queryability, stable intent and agent-native editing.
- Native Studio inspector edit proven end-to-end through real macOS text entry: source changed, preview/bounds updated, `LAST CHANGE` reported the semantic resize, and source was restored cleanly.
- Native Studio image import proven through the actual file-dialog journey.
- Structured UI screenshot import produced 9 editable stable-ID objects.
- Imported reconstruction vs 430×932 source benchmark: **0.9215% changed pixels**, mean channel delta **1.1567/255**. This is bounded UI evidence only, not arbitrary-photo reconstruction.
- Editing imported `Today` → `Tomorrow` changed exactly one authored semantic field/object; no unrelated stable object changed.
- Commander plugin-runner Node/PATH defect fixed: Glyph now resolves Node through `GLYPH_NODE_BIN`, `NODE_BINARY`, process `PATH`, then standard Homebrew/system locations.

Key evidence file: `docs/CORE_GATE_REPORT.md`.

## Approximate completion

Use these percentages only as planning estimates, not test scores:

- Core architectural/MVP proof: **~88% complete**.
- Full original research vision: **~62% complete**.
- Polished public Glyph 1.0 product: **~50–55% complete**.

The central architecture is validated. Most remaining engineering weight is in language hardening, richer graphics, production typography, full Studio direct manipulation, broader inverse graphics/Image→Glyph, motion, accessibility, modules/tooling and production release hardening.
## What to build next — dependency order

Do not jump straight to arbitrary photo reconstruction or cosmetic Studio features. Continue in this order unless a newly discovered blocker changes the dependency graph.

### Phase 1 — Freeze the language/GSIR v1 contract

1. Add a real dimensional type system: `Length`, `Angle`, `Color`, `Opacity`, `Point`, `Vector`, `Rect`, `Path`, `Image`, `Font`, `Percentage`, `Depth`, `Time` and related safe operations.
2. Add explicit units and reject dimensionally invalid expressions such as length + angle.
3. Define schema/version migration rules for `.glyph` and GSIR.
4. Finish typed component parameters, slots, variants and override semantics.
5. Define modules/imports/project boundaries without recreating npm.
6. Expand canonical primitives carefully: images, masks/clips, transforms, gradients/patterns/material/effects only where the semantic model remains clean.

### Phase 2 — Production rendering + typography

1. Replace estimated text metrics with CoreText/HarfBuzz-class shaping and real font measurement.
2. Add font fallback, kerning, ligatures, variable fonts and deterministic multiline behavior.
3. Make renderer conformance tests across SVG/CommanderDraw and future direct targets.
4. Add production PDF/SwiftUI/Web/Canvas export paths where semantic export makes sense.
5. Keep pixel/vector fidelity exports separate from semantic code-generation exports.

### Phase 3 — Complete Studio round-trip editing

1. Direct drag/resize manipulation that rewrites semantic Glyph intent, not SVG coordinates.
2. Constraint handles and visual constraint creation/editing.
3. Multi-select, hierarchy reordering, grouping and component operations.
4. Undo/redo with source/GSIR revision integrity.
5. Better semantic visual-diff presentation and conflict diagnostics.

### Phase 4 — Broaden Image → Glyph carefully

1. General segmentation and region decomposition.
2. Hybrid vector/raster regions, masks and images.
3. Depth/order and uncertainty representation.
4. Organic-shape proposals and editable geometry.
5. Iterative reconstruction against a reference image.
6. Keep uncertain recognition as confidence-bearing suggestions; never silently convert model guesses into authoritative design intent.

### Phase 5 — Motion, accessibility, tooling and production hardening

Finish timeline/state/spring semantics, accessibility in GSIR, LSP/autocomplete/diagnostics, packages/assets/project bundles, migrations, fuzzing, performance profiling, release packaging and security review.
## Non-negotiable architecture rules

- **GSIR is canonical.** The surface DSL is replaceable; the semantic IR is the center.
- **CommanderDraw is a backend/compatibility/export layer**, not Glyph's source of truth.
- Do not turn Glyph into a generic application language.
- Do not bind semantic roles to a closed AI-recognition ontology. Keep roles/tags open-ended.
- Preserve stable IDs across edits, lowering and rendering wherever the target supports identity.
- Preserve authored intent separately from solved geometry.
- Preserve provenance and uncertainty for inferred/vision-derived content.
- Hard geometric constraints outrank heuristic/perceptual objectives; learned critique is advisory.
- Do not claim visual fidelity from validator scores or schema correctness. Inspect rendered pixels.
- Do not claim arbitrary-image/photo reconstruction is solved from the bounded UI benchmark.
- Do not use Figma as the primary architecture. The user explicitly prefers native Glyph/CommanderDraw workflows; Figma is optional interoperability only.
- Prefer surgical source/semantic edits. Redraw/restructure when the governing design relationships are wrong rather than accumulating patches.

## Current repo caution

Several older docs still contain statements from earlier v0.1 stages, for example saying the original solver, Studio or Image→Glyph is not implemented. Before relying on a limitation statement, compare it with:

1. `glyph capabilities`;
2. `docs/CORE_GATE_REPORT.md`;
3. this handoff;
4. actual source/tests.

Do not "correct" the repo by reverting newer implementation to match stale prose.

## Startup procedure for the next chat

1. Read the five files listed in the bootstrap prompt.
2. Run `git status` and inspect the existing working tree; preserve it.
3. Run live `glyph capabilities`.
4. Run `cargo fmt --check` and `cargo test`; expected baseline at this handoff is 92 passing Rust integration tests, plus 4 Studio integration tests through the full verification script.
5. Build the release binary if source has changed.
6. Create a non-destructive Commander Git snapshot/checkpoint before the next substantive implementation block.
7. Continue the remaining **Phase 3 Studio round-trip work** in dependency order: text-specific semantic resize and multi-edge handles are the next direct-manipulation gaps; structural group membership/translation and broader component/path inverse cases follow. Do not reopen completed Phase 1/2 work unless a real regression requires it.
8. After each meaningful subsystem, rerun the unchanged core gate and relevant targeted benchmark. Do not weaken a benchmark because a new feature fails it.

## Useful paths

- Core source: `src/`
- Native Studio: `studio/`
- Core benchmark runner: `benchmarks/run_core_gate.py`
- StepIsland comparison: `benchmarks/stepisland/`
- Vision gate: `scripts/verify_vision.sh`
- Architecture: `docs/ARCHITECTURE.md`
- MVP: `docs/MVP.md`
- Fixed benchmark criteria: `docs/BENCHMARK_PLAN.md`
- Verified evidence: `docs/CORE_GATE_REPORT.md`
- Full research archive: `docs/GLYPH_RESEARCH_ARCHIVE.md`

## Definition of "continue"

When the user says `continue @Commander`, do not repeat research or ask what to do. Read the current state, pick up the next unfinished item in the dependency-ordered roadmap, implement it, test it, and leave evidence. If a real defect appears, treat the gate as failed until fixed rather than redefining success.
