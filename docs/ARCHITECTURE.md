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
5. **Output layer** — SVG plus CommanderDraw compatibility/rendering.

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

Scene solves currently rebuild the solver per invocation rather than retaining a long-lived incremental edit session. That implementation detail remains isolated behind the solver boundary and does not change `.glyph`, GSIR, Commander tools, or patch transactions.

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
- Constraints are now first-class authored edit targets. `glyph constraints`, `constraint-edit`, `constraint-remove`, and `constraint-add` operate by source order, preserve unrelated source, and semantically reparse the result so dimensionally invalid relations fail closed.
- Studio projects constraint badges onto the selected object's resolved bounds and provides explicit relation, strength, offset, remove, and add controls in the intent inspector. Candidate-solve validation resolves compatible multi-constraint intent automatically; incompatible or unsafe intent still refuses to guess, and the author can explicitly change the constraint set and retry.

Phase F direct-manipulation now includes text-frame resize, multi-edge handles, structural group membership/translation, compatible path translate-constraint inversion, and explicit component-owned affine path inversion. Remaining inverse-graphics work is intentionally narrower: rotated-path semantics and mixed/ambiguous component-constraint ownership still fail closed rather than guessing.

## Rendering

Glyph does not contain a custom rasterizer. The resolved scene now lowers first into a deterministic **backend-neutral display list**. That list carries stable IDs, roles, resolved primitives, paint, bounds and source provenance plus a stable object map.

- SVG consumes the display list and preserves `id` and `data-glyph-role` metadata.
- CommanderDraw compatibility consumes the same display list before PNG rendering.
- `glyph conformance <scene>` reports backend semantics as `exact`, `degraded`, or `unsupported` with stable item IDs and reasons. This is a semantic contract, not a pixel-equivalence claim.
- `.draw.json` remains an output bridge, not canonical Glyph state.
- Future Skia/Metal/WebGPU renderers should consume this display list rather than reinterpret GSIR independently.

### Target contract

Glyph separates **visual export** from **semantic code generation**. They are not interchangeable promises.

- Implemented visual exports: SVG, CommanderDraw JSON, PNG, and PDF. PDF lowers through CommanderDraw and fails closed when that bridge cannot preserve required visual semantics.
- `glyph targets <scene>` reports target class, implementation availability, current conformance, lowering path, semantic-ID preservation, editability, and limitations for the actual scene.
- SwiftUI and Web/Canvas are currently `strategy_only` semantic-codegen targets. No emitter exists yet, so Glyph reports them as unsupported rather than generating misleading native code.
- A future codegen emitter must operate on a documented display-list subset and must never claim arbitrary illustration can round-trip as readable native UI code.

### Language service and LSP

Glyph editor tooling is a projection of the production compiler contracts, not a second parser. `src/language_service.rs` turns the real parser, solver and accessibility audit into deterministic diagnostics and supplies context-aware completions for scene items, node properties, group members and constraint references. Parse diagnostics retain source line/column information; semantic diagnostics reuse solver and accessibility findings.

For real project files, diagnostics resolve the normal local import graph while overlaying the unsaved entry-document text through `load_project_scene_with_source`; imported components/styles/paints therefore keep the same project-boundary and namespace rules as compilation. `glyph diagnostics` exposes the diagnostic model as JSON and `glyph complete` exposes completion candidates directly.

`glyph lsp` implements a stdio JSON-RPC Language Server Protocol baseline with initialize/shutdown/exit, full-document open/change/close synchronization, publishDiagnostics and completion. The first LSP contract is intentionally synchronous and dependency-light. Incremental text sync, references/rename, code actions and richer semantic navigation are future tooling layers; they must reuse the canonical semantic model rather than inventing editor-only meaning.

## Commander API

The v0.1 plugin exposes read and mutation operations separately:

- discover: `capabilities`, `query`, `inspect`, `why`, `object-map`, `targets`, `drag-plan`, `resize-plan`, `constraints`;
- validate: `check`, `test`, `conformance`;
- mutate: `compile`, `apply`, `source-set`, `source-drag`, `source-resize`, `constraint-edit`, `constraint-remove`, `constraint-add`;
- compare: `diff`, `visual-diff`;
- render/export: display-list JSON, SVG, PNG, PDF, CommanderDraw JSON.

Normal agent workflow:

```text
query role → inspect/why → apply transaction → solve → diff → render → visual critique
```

## Post-gate roadmap state

Completed after the fixed effectiveness gate:

1. Cassowary-class weighted linear solver, while retaining the original resolver as a legacy comparison path;
2. reusable styles, components/instances and seeded deterministic repeat;
3. declarative assertions, named states and sampled motion v1;
4. surgical `.glyph` source property round-trip editing;
5. backend-neutral display list + object map now used by both SVG and CommanderDraw output;
6. CoreText shaping, fallback/substitution diagnostics, ligature/spacing controls and validated variable-font axes;
7. renderer conformance plus the target strategy contract, with production PDF export and fail-closed unsupported semantics;
8. Phase F direct-manipulation v1: source-authoritative drag/resize planning, compatible multi-equality co-editing, inequality feasibility guards, affine path translation and unrotated path resize without rewriting `d`, rect/image/ellipse semantic resize, unambiguous typed component-parameter inverse editing, first-class constraint query/edit/add/remove with Studio badges and controls, native Studio handles, and validated source-snapshot undo/redo.

Remaining Phase F hardening is now concentrated on rotated/ambiguous inverse graphics, deeper Studio hierarchy/component operations, richer LSP navigation/actions beyond the implemented diagnostics/completion baseline, assets/project packaging, fuzzing, performance/security/release work, and the still-gated SwiftUI/Web Canvas semantic emitters. Accessibility semantics/auditing and the first production-linked language-service/LSP baseline are implemented.
