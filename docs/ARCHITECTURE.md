# Glyph Architecture

## Core contract

Glyph treats **design intent as the unit of editing**. A source statement, GSIR node, resolved visual object, and rendered output remain linked by stable identity and provenance.

```text
source span ↔ typed node ↔ GSIR stable ID ↔ resolved object ↔ rendered element
```

## Layers

1. **Language** — compact declarative `.glyph` syntax for humans and agents.
2. **GSIR** — canonical semantic scene representation.
3. **Constraint layer** — authored relationships and priorities.
4. **Resolved scene** — derived numeric geometry after solving.
5. **Output layer** — deterministic SVG visual export plus bounded semantic code generation.

The DSL is replaceable. GSIR is the architectural center.

## Version boundaries

`.glyph` source syntax, GSIR serialization, and scene edit revision are versioned independently. Source v1 may be declared with `@version(1)`; GSIR currently writes schema v2; migrations never increment scene revision. Future source/schema versions fail closed. The full compatibility policy is in `docs/SCHEMA_VERSIONING.md`.

Typed component interfaces are part of source v1 and GSIR v2: parameters, value slots, one selected named variant, and local property overrides resolve deterministically while preserving `<instance>/<template>` stable IDs. See `docs/COMPONENTS.md` for syntax and precedence.

## GSIR node model

Each addressable node contains:

- `id`: stable identity used by agents, diffs and render metadata;
- `kind`: small rendering-oriented structural kind (`rect`, `ellipse`, `path`, `text`, `image`, `group`);
- `roles[]`: open semantic tags such as `tree`, `foreground`, `mascot`;
- `authored.props`: values explicitly chosen by the author/agent;
- `derived.props`: solved geometry generated from authored state + constraints;
- `provenance`: source path and line;
- `confidence`: reserved for future inferred/vision-derived information.

Semantic classes such as `tree` are **not compiler enums**. This prevents the language from becoming coupled to one AI model's ontology.

## Authored vs derived

If source says:

```glyph
constrain mascot.center_x == canvas.center_x @required;
```

and the canvas changes width, the solver may change `mascot.derived.cx`. It does not overwrite an authored `cx` declaration or replace the relation with a literal coordinate. This separation is required for future source↔canvas round-tripping.

## Constraints

The IR distinguishes `required`, `strong`, `medium`, and `weak` constraints. The current resolver uses a Cassowary-class weighted linear solver for equality and inequalities over common geometric properties and records relaxed/unresolved/conflicting relations.

One-shot `solve_scene` remains available, while Studio uses a long-lived `LinearSolverSession`. Topology-stable refreshes reuse Cassowary variables/constraints and update authored stays, CoreText metrics, canvas dimensions, and source-constraint values in place; structural topology changes rebuild deterministically. Incremental results are regression-tested against cold solves and do not change `.glyph`, GSIR, Commander tools, or patch transactions.

Future solving tiers should remain separate:

```text
required geometry
  ↓
soft linear intent
  ↓
deterministic procedural layout
  ↓
optional perceptual objectives
  ↓
advisory AI critique
```

Perceptual heuristics must never override required geometry.

## Agent transactions

Commander should modify Glyph through transactions rather than blind text replacement.

A transaction includes:

- `base_revision`;
- one or more typed operations;
- an output revision.

Glyph rejects stale base revisions. After applying operations it re-solves constraints and reports:

- changed authored fields;
- unrelated authored nodes changed;
- required constraint violations;
- relaxed/unresolved constraints.

This is the first concrete mechanism for the project's central promise: **surgical edits with measurable collateral change**.

## Direct manipulation v1

Studio drag is source-authoritative rather than a post-render coordinate patch. `glyph drag-plan` first decides what authored intent represents the requested movement; `glyph source-drag` applies only an allowed plan through the source round-trip editor.

- Rect, image and text nodes author `x`/`y`; ellipses author `cx`/`cy`; paths move through authored `translate_x`/`translate_y` affine properties so their SVG `d` geometry remains untouched. Structural groups now own validated `member` references; nested group bounds are unions of leaf bounds, and group drag composes per-leaf semantic plans while preserving translation-invariant internal relations.
- A free axis edits its direct authored position while preserving explicit `px` units.
- Positional equality constraints on a manipulated axis are co-edited deterministically when the candidate solve proves the requested motion exactly. Existing unitless or `px` offsets remain in the same unit contract; unsupported offset units fail closed.
- Inequalities are preserved as guards rather than rewritten. A gesture proceeds only when the candidate solve keeps those guards satisfied and does not newly relax unrelated authored constraints; otherwise it fails closed with exact constraint IDs.
- A node referenced only on another object's RHS remains movable, so dependent geometry can re-solve from the retained relation.
- Generated nodes fail closed. Component-instance children can be manipulated when the affected local property maps to exactly one typed instance parameter; the inverse edit updates or inserts that `param` value, compensates for instance scale, and leaves the component definition untouched. Instance overrides, ambiguous multi-parameter bindings, and invalid scale fail closed.
- If any requested axis cannot be represented safely, the whole two-axis gesture is rejected rather than silently applying a partial drag. Otherwise a single gesture may combine a direct coordinate edit on one axis with a relation-offset edit on the other.
- Studio undo/redo stores validated `.glyph` source snapshots. Restores run `glyph check` before replacing the live file and then re-solve/re-render; canvas-only state is never treated as authoritative history.
- Manual unsaved source text disables canvas manipulation and semantic undo/redo until saved, avoiding conflicts between text-editor state and semantic history.
- Rect/image/ellipse semantic resize uses the same source-authoritative contract: free dimensions edit `width`/`height` or ellipse `rx`/`ry`; compatible size equalities are co-edited; inequalities remain feasibility guards; unsupported units, newly relaxed constraints, or non-positive results fail closed.
- Unrotated authored paths can resize without rewriting `d`: the planner adjusts explicit `scale_x`/`scale_y` plus translation so the resolved anchor stays fixed and validates exact bounds. Compatible authored `translate_x`/`translate_y` equalities are inverted through relation-offset edits, while unsupported scale/geometry constraints or RHS dependencies fail closed. Component-owned paths resize only when every affected affine property has one explicit parameter binding; mixed component/constraint ownership and rotated/degenerate/negative-scale cases fail closed.
- Studio exposes explicit left/right/top/bottom/corner semantic handles for rect, image, ellipse, and path nodes, and left/right wrap-frame handles for text. Each handle preserves the opposite edge/corner through a composed size + movement plan and exact candidate-bounds validation.
- Structural hierarchy edits are source-authoritative too: `group-create`, `group-add-member`, `group-remove-member`, `group-remove`, and `node-reparent` preserve unrelated source, enforce single ownership/cycle rules, and reparse the edited source before commit. `hierarchy` exposes directly-authored layer owners in source/render order plus explicit parent/member relationships; `node-reparent` moves a direct authored node/group between structural parents or the scene root without changing declaration/paint order, while `node-move` performs validated owner reordering. Component children resolve to their whole instance owner so an instance cannot be split by a paint-order edit. The project-aware `components` catalog exposes typed parameters/slots/variants; `instance-add`, `instance-remove`, `instance-param-set`, and `instance-slot-set` mutate authored instance intent and validate the full project/solver candidate before writing. `component-extract` moves a contiguous selection of directly-authored leaf layers into a new local component definition and replaces them with an instance at the same paint-order position; it preserves visual geometry and fails closed if constraints, group membership, clip/mask/accessibility links, non-contiguous order, existing instances, or other node references would require implicit namespace migration. Studio exposes this as **Make Component…** alongside multi-selection grouping/ungrouping, explicit parent reassignment, backward/forward owner reordering, and the component library, all through validated source undo/redo.
- Constraints are now first-class authored edit targets. `glyph constraints`, `constraint-edit`, `constraint-remove`, and `constraint-add` operate by source order, preserve unrelated source, and semantically reparse the result so dimensionally invalid relations fail closed.
- Studio projects constraint badges onto the selected object's resolved bounds and provides explicit relation, strength, offset, remove, and add controls in the intent inspector. Candidate-solve validation resolves compatible multi-constraint intent automatically; incompatible or unsafe intent still refuses to guess, and the author can explicitly change the constraint set and retry.

Phase F direct-manipulation now includes text-frame resize, multi-edge handles, structural group membership/translation, compatible path translate-constraint inversion, and explicit component-owned affine path inversion. Remaining inverse-graphics work is intentionally narrower: rotated-path semantics and mixed/ambiguous component-constraint ownership still fail closed rather than guessing.

## Motion runtime

Motion remains source-authoritative and deterministic. Animation specs carry typed `s`/`ms` duration and delay, finite repeat counts, optional alternate playback and a bounded set of deterministic ease/spring presets. Sampling at a given time produces a resolved scene without rewriting authored source; negative/non-finite sample times and invalid repeat/easing declarations fail closed. These spring presets are deterministic interpolation functions, not a continuously integrated physics engine.

## Vision / inverse graphics

Image→Glyph keeps deterministic authored state separate from uncertain recognition. `glyph import-image` accepts `--mode structured|hybrid|auto`: structured mode emits editable text/shape reconstruction for high-confidence UI-like imagery; hybrid mode preserves the original image as a full-canvas raster node and keeps detected semantic objects as zero-opacity, assistive-hidden, confidence-bearing overlays. `auto` chooses between those bounded contracts deterministically from manifest density/confidence. The hybrid path preserves pixels instead of pretending ambiguous photographs or illustrations are clean vectors; arbitrary photo-to-clean-vector reconstruction remains a non-goal.

## Rendering

Glyph does not contain a custom rasterizer. The resolved scene now lowers first into a deterministic **backend-neutral display list**. That list carries stable IDs, roles, resolved primitives, paint, bounds and source provenance plus a stable object map.

- SVG consumes the display list and preserves `id` and `data-glyph-role` metadata.
- SVG is the deterministic visual export over the backend-neutral display list.
- `glyph conformance <scene>` reports backend semantics as `exact`, `degraded`, or `unsupported` with stable item IDs and reasons. This is a semantic contract, not a pixel-equivalence claim.
- Future Skia/Metal/WebGPU renderers should consume this display list rather than reinterpret GSIR independently.

### Target contract

Glyph separates **visual export** from **semantic code generation**. They are not interchangeable promises.

- Implemented visual export: SVG. SwiftUI and Web Canvas are separate bounded semantic-codegen targets and fail closed when the scene uses unsupported semantics.
- `glyph targets <scene>` reports target class, implementation availability, current conformance, lowering path, semantic-ID preservation, editability, and limitations for the actual scene.
- SwiftUI and Web Canvas are implemented semantic-codegen targets over the bounded subset documented in `docs/CODEGEN.md`. `glyph targets` evaluates the actual scene and reports `exact`, `degraded`, or `unsupported`; emitters refuse unsupported semantics rather than flattening them into misleading native code.
- Codegen currently covers rect/ellipse/group/basic-text scenes with simple solid styling and stable identity. Paths, images, affine transforms, clips/masks, gradients, advanced typography and interaction contracts remain outside the subset until their native lowering/ownership contracts are explicit. Arbitrary illustration is never claimed to round-trip as readable native UI code.

### Language service and LSP

Glyph editor tooling is a projection of the production compiler contracts, not a second parser. `src/language_service.rs` turns the real parser, solver and accessibility audit into deterministic diagnostics and supplies context-aware completions for scene items, node properties, group members and constraint references. Parse diagnostics retain source line/column information; semantic diagnostics reuse solver and accessibility findings.

For real project files, diagnostics resolve the normal local import graph while overlaying the unsaved entry-document text through `load_project_scene_with_source`; imported components/styles/paints therefore keep the same project-boundary and namespace rules as compilation. Project navigation builds a bounded index of local `.glyph` files under the discovered project root and resolves component/style/paint symbols through the same alias chains, including transitive namespaces. Open document buffers overlay disk content so navigation and refactors operate on the editor's current source rather than stale files. `glyph diagnostics` exposes the diagnostic model as JSON and `glyph complete` exposes completion candidates directly.

`glyph lsp` implements a stdio JSON-RPC Language Server Protocol service with initialize/shutdown/exit, UTF-16-safe incremental open/change/close synchronization, publishDiagnostics, completion, definition, references, hover, full semantic tokens, prepare-rename/rename and quick-fix code actions. Top-level node navigation/rename remains document-scoped so stable `@id(...)` identity and local ownership are preserved. Exported component/style/paint definition/references, imported-symbol hover and rename are project-aware: transitive import aliases resolve back to the declaring file, and one rename returns a deterministic multi-file workspace edit while preserving namespace prefixes. Ambiguous or colliding exported names fail closed. Hover text and semantic token classes are projections of canonical compiler meaning rather than editor-only syntax. The first code-action contract remains deliberately narrow: it can add a deterministic accessible name for the corresponding accessibility diagnostic. Richer refactors/code actions and deeper editor projections remain future layers.

### Project assets and bundles

A `glyph.project.json` manifest can declare the canonical entry point plus named assets with required SHA-256 digests. `project-check` resolves the same bounded import graph used by compilation, rejects project-root escapes and mismatched entry declarations, hashes the transitive source closure, and verifies every declared asset before reporting success. `project-bundle` copies only that verified dependency closure into a staging directory, writes canonical `glyph.project.json` and `glyph.bundle.json` manifests, re-validates the staged project, then atomically publishes the bundle directory. Existing output paths are never overwritten.

The public package-registry problem remains separate. Glyph now has deterministic local packaging and content-addressed assets without introducing remote dependency resolution, lockfile registries or npm-style package semantics prematurely.

### Release hardening

`tests/robustness.rs` applies a deterministic parser mutation corpus and malformed-edit fail-closed checks. `scripts/profile_core.sh` enforces bounded release-mode check/compile time for a 5,000-node synthetic scene; `scripts/security_check.sh` composes robustness, project-containment and asset-hash regressions plus `cargo-audit` when available; `scripts/package_release.sh` produces checksummed macOS CLI/Studio/Vision archives; and `scripts/verify_release.sh` runs the complete source, Studio, hardening, performance and packaging path.

## Commander API

The v0.1 plugin exposes read and mutation operations separately:

- discover: `capabilities`, `query`, `inspect`, `why`, `object-map`, `targets`, `drag-plan`, `resize-plan`, `constraints`;
- validate: `check`, `project-check`, `test`, `conformance`;
- mutate/package: `compile`, `project-bundle`, `apply`, `source-set`, `source-drag`, `source-resize`, `constraint-edit`, `constraint-remove`, `constraint-add`, structural group/member operations, `node-reparent`, `node-move`, `component-extract`, and component-instance create/remove/binding edits;
- compare: `diff`, `visual-diff`;
- render/export: display-list JSON and deterministic SVG, plus fail-closed bounded SwiftUI and Web Canvas source emitters.

Normal agent workflow:

```text
query role → inspect/why → apply transaction → solve → diff → render → visual critique
```

## Post-gate roadmap state

Completed after the fixed effectiveness gate:

1. Cassowary-class weighted linear solver, while retaining the original resolver as a legacy comparison path;
2. reusable styles, components/instances and seeded deterministic repeat;
3. declarative assertions, named states and deterministic sampled timeline motion with typed delay/repeat/alternate/ease/spring semantics;
4. surgical `.glyph` source property round-trip editing;
5. backend-neutral display list + object map used by SVG export, semantic inspection and target assessment;
6. CoreText shaping, fallback/substitution diagnostics, ligature/spacing controls and validated variable-font axes;
7. SVG renderer conformance plus the target strategy contract, with fail-closed unsupported semantic codegen;
8. Phase F direct-manipulation v1: source-authoritative drag/resize planning, compatible multi-equality co-editing, inequality feasibility guards, affine path translation and unrotated path resize without rewriting `d`, rect/image/ellipse/text-frame semantic resize, explicit multi-edge handles, structural groups, unambiguous typed component/path inverse editing, first-class constraint query/edit/add/remove with Studio badges and controls, and validated source-snapshot undo/redo;
9. first-class accessibility semantics/auditing lowered through display-list and SVG ARIA metadata;
10. production-linked diagnostics/completion plus UTF-16-safe incremental LSP navigation, document-local node rename, transitive project-aware component/style/paint definition/references/cross-file rename, and accessibility quick fixes;
11. structured/hybrid/auto Vision import so ambiguous imagery preserves raster pixels instead of inventing vector certainty;
12. content-addressed local project assets plus deterministic staged project bundles over the transitive source dependency closure;
13. deterministic mutation robustness tests, 5,000-node performance budgets, containment/hash security gates and checksummed macOS release packaging;
14. bounded SwiftUI and Web Canvas semantic code generation with per-scene conformance assessment, stable identity and fail-closed unsupported semantics;
15. source-authoritative local component extraction for contiguous directly-authored leaf layers, preserving visual geometry/order and failing closed rather than silently migrating cross-node namespaces; Studio exposes the same operation with source-snapshot undo.

The named Phase F implementation baseline is complete. Post-baseline extensions remain deliberately gated by explicit semantics rather than treated as missing infrastructure: rotated-path inverse resize, mixed/ambiguous component-constraint ownership, richer component restructuring that would require automatic parameterization/reference migration, editor projections beyond the implemented structural parent/member projection and cross-file symbol index/refactors, cross-platform typography parity, and broader native-code subsets. Arbitrary photo-to-clean-vector reconstruction, a public remote package registry and learned aesthetic constraints as authority remain deliberate non-goals unless the product scope changes.
