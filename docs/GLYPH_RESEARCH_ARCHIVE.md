# Glyph — Canonical Research Archive

## Purpose of this file

This file preserves the **full research problem, conclusions, architecture, product definition and roadmap** that led to Glyph. It is intended to survive chat turnover and serve as the canonical research reference for future implementation work.

It is a consolidated archive reconstructed from the original deep-research brief, the completed research conclusions, and the verified implementation work that followed. It is **not claimed to be a verbatim transcript of every sentence from the original research response**; it preserves the substantive research content and decisions so no architectural context is lost.

## Executive conclusion

Glyph should **not** primarily be treated as “a new graphics language.” The strongest opportunity is a **semantic visual intermediate representation + constraint system + deterministic compiler/runtime**, with a compact DSL as the human/agent-facing authoring layer.

The architectural center is therefore:

```text
Glyph source
  ↓
semantic typed AST / canonical GSIR
  ↓
relationships + constraints + authored intent
  ↓
resolved geometry / display list
  ↓
existing proven rendering backends
  ↓
SVG / PNG / PDF / SwiftUI / Web / Canvas / other targets
```

The important innovation is not a different spelling for SVG. It is preserving **stable semantic identity, relationships, intent, provenance, constraints, editability and agent-addressable structure** across authoring, solving, rendering and round-trip editing.
## The question the research had to answer

If Glyph were built today so an AI agent could design and modify sophisticated visual assets **significantly more reliably** than raw SVG, image generation, Figma automation, SwiftUI or the original CommanderDraw model, what exact architecture should be built first, what should be reused, and what should be postponed?

The research was required to actively try to prove Glyph unnecessary. If existing tools plus a thin semantic layer were enough, the conclusion should have been to avoid inventing a language.

The answer was nuanced:

- A brand-new renderer is unnecessary.
- A language syntax alone is insufficient.
- Existing renderers and graphics libraries are valuable and should be reused.
- A semantic visual IR with stable intent, constraints and bidirectional editability **is** a meaningful missing layer.
- Therefore Glyph is justified only if it remains a semantic/constraint/runtime system rather than becoming a vanity syntax or another canvas API.

## Core product thesis

A design system becomes dramatically easier for AI agents to edit when the scene can answer questions such as:

- Which object is the mascot?
- Which tree is the right-side foreground tree?
- Why is this label centered?
- What moved because the canvas resized?
- What authored value did the user choose versus what value was derived by a solver?
- Which relationship would break if this object moved?
- Which object changed between revisions?
- Can one object be changed without modifying unrelated authored state?

Raw pixels, anonymous paths and imperative drawing calls do not answer those questions naturally. Glyph exists to preserve those answers as first-class state.

## Forward and inverse pipelines

Forward pipeline:

```text
Glyph source
→ semantic visual scene graph / GSIR
→ constraint/composition solver
→ resolved geometry / display list
→ deterministic renderer/exporter
→ PNG / SVG / PDF / SwiftUI / Web / Canvas / other targets
```

Inverse pipeline:

```text
image / screenshot / illustration / photo
→ visual understanding
→ object / region / relationship decomposition
→ confidence-bearing semantic scene proposal
→ editable GSIR
→ editable Glyph source
```

The inverse pipeline must never pretend uncertainty is certainty. Recognition, decomposition and organic geometry are suggestions with provenance/confidence; deterministic authored state becomes authoritative only after acceptance/editing.
## Prior-art landscape

The research surveyed graphics, UI, procedural design, visual programming, layout, typography and inverse-graphics systems rather than treating Glyph as isolated.

### Low-level graphics and document models

Relevant systems: SVG, PostScript, PDF, Canvas, OpenGL, WebGL, WebGPU, Metal, Skia, Cairo and Core Graphics.

Research takeaway:
- They are strong rendering/geometry substrates.
- They are usually too low-level to preserve semantic design intent by themselves.
- Glyph should compile **to or through** these systems, not replace their rendering engines.
- SVG is particularly useful as an editable interchange/output, but it should not be the canonical semantic model.

### Declarative UI/layout systems

Relevant systems: SwiftUI, Jetpack Compose, Flutter, React, CSS, Auto Layout, Yoga and Taffy.

Research takeaway:
- Declarative composition and reactive layout are useful precedents.
- SwiftUI demonstrates that semantic variables and relational layout can already outperform raw coordinate code in some responsive cases.
- Glyph must therefore win on visual-domain semantics, stable cross-target identity, constraint introspection, agent patchability and round-trip editing—not merely “declarative syntax.”
### Creative coding and vector libraries

Relevant systems: Processing, p5.js, OpenFrameworks, Cinder, Paper.js, Two.js and Three.js.

Takeaway:
- Excellent for procedural drawing and custom visuals.
- Usually imperative or geometry-centric.
- Glyph should borrow composability and procedural power while keeping semantic object identity and relational intent explicit.

### Design tools and product-design runtimes

Relevant systems: Figma APIs/variables, Framer, Origami, Penpot, Sketch, Rive, Lottie, Haiku, Plasmic and Webflow.

Takeaway:
- Components, variants, tokens, interactions and direct manipulation are proven product concepts.
- Existing design tools usually make the canvas/editor primary and code/agent semantics secondary.
- Glyph should invert that relationship: semantic source/IR remains authoritative and the canvas is a projection/editor onto it.
- Figma interoperability can exist, but Glyph should not depend on Figma as its core editing/runtime model.

### Procedural/CAD/node systems

Relevant systems: Grasshopper/Rhino, Houdini, Blender Geometry Nodes, OpenSCAD and CAD constraint solvers.

Takeaway:
- These systems prove the value of explicit relationships, procedural generation, parameterization and graph-based intent.
- Glyph should adopt hard/soft constraints, parameters and deterministic procedural generation while avoiding CAD-level complexity for ordinary visual design.
### Diagram/document languages

Relevant systems: Mermaid, Graphviz, D2, PlantUML, TikZ, Typst and LaTeX.

Takeaway:
- Domain-specific visual languages can be extremely productive when the semantic domain is constrained.
- Glyph needs a broader visual model than diagram DSLs while preserving their determinism, diffability and code-review friendliness.

### Metafont / METAPOST and parametric typography

Metafont and METAPOST were especially important precedents because they treat visual form as a system of relationships and parameters rather than isolated coordinates.

Takeaway:
- Parametric visual construction is a strong conceptual ancestor for Glyph.
- The modern extension is stable semantic object identity, component systems, agent operations, provenance, richer rendering targets and round-trip visual editing.

### Typography engines

Relevant systems: HarfBuzz, CoreText, Skia text and Pango.

Takeaway:
- Glyph should not reinvent complex shaping.
- Typography belongs deeply in the semantic/layout model, but shaping and font-system behavior should delegate to mature engines.

### Inverse graphics and structured visual understanding

Research scope included raster-to-vector work, differentiable rendering, segmentation, object detection, depth estimation, scene graphs, OCR/document understanding, screenshot/layout extraction, structured generation and neural scene representations.

Takeaway:
- Image→Glyph is plausible as a **hybrid reconstruction pipeline**, not as perfect universal vectorization.
- Structured screenshots/UI are much easier than arbitrary photography or painterly illustration.
- Confidence, provenance and unresolved ambiguity must be first-class.
## Product definition

Glyph is best defined as an **agent-native programmable design format and semantic visual runtime**.

It should provide:
- a compact source language;
- a canonical semantic scene IR;
- stable object identity;
- constraints and relational composition;
- reusable styles/components/parameters;
- deterministic procedural design;
- source provenance and revisioning;
- renderer-independent resolved visual state;
- semantic and visual diffing;
- agent APIs for query/inspect/patch/why/render;
- a native visual editor that round-trips to the same source/IR;
- bounded AI/vision assistance where deterministic rules alone are insufficient.

## Explicit non-goals

Glyph should not become:
- a replacement for Skia/Core Graphics/Metal/WebGPU;
- a new general-purpose programming language for building whole applications;
- a Figma clone;
- a giant closed semantic ontology;
- a prompt-only generative-image wrapper;
- an arbitrary-photo-to-perfect-vector system;
- a system that reduces aesthetics to one numeric “beauty score”;
- a package ecosystem reinventing npm without necessity.

The boundary is important: **host applications own application logic and state; Glyph owns visual scene representation, relationships, rendering intent and visual-state transitions.**
## Canonical semantic model

The research proposed a layered model rather than a flat list of drawing commands:

```text
Semantic → Composition → Constraint → Geometry → Rendering
```

Candidate scene concepts included:
- `Scene`
- `Object`
- `Group`
- `Region`
- `Shape`
- `Surface`
- `Text`
- `Image`
- `Texture`
- `Material`
- `Light`
- `Mask`
- `Path`
- `Pattern`
- `Symbol`
- `Component`
- `Layout`
- `Constraint`
- `Relationship`
- `Style`
- `Effect`
- `Camera`
- `DepthPlane`
- `Reference`

The key design decision is to separate **rendering kind** from **semantic role**. A tree should not require `Tree` to be a compiler enum. It may be a group/path/shape with roles such as `tree`, `foreground`, `organic`, `focal-support`. This keeps Glyph independent from one recognition model or ontology.

A stable node needs at minimum:
- stable `id`;
- structural/render kind;
- free-form semantic roles/tags;
- authored properties;
- derived/solved properties;
- provenance;
- optional confidence/uncertainty;
- component/style relationships;
- relationships/constraints affecting it.
## Relationships as first-class design state

The research emphasized that visual relationships are often more important than literal coordinates.

Examples:
- centered in;
- aligned with;
- left/right/above/below;
- inside/contains;
- overlaps/crosses;
- in front of / behind;
- same width/height/radius;
- attached to;
- evenly distributed;
- balanced against;
- subordinate to / focal relative to;
- preserves negative space around;
- optical center rather than geometric center.

These relationships must be classified rather than all treated as identical constraints.

### Deterministic formal constraints

Suitable for exact solving:
- equality/inequality;
- alignment;
- containment;
- min/max dimensions;
- aspect ratio;
- spacing;
- anchors;
- relative positions;
- shared dimensions.

### Heuristic optimization

Suitable for deterministic or bounded optimization:
- packing;
- collision avoidance;
- force-like distribution;
- optical balance;
- saliency-weighted spacing;
- line-break/layout trade-offs.

### Learned/advisory suggestions

Suitable for optional AI critique rather than language truth:
- “feels visually heavy”;
- “composition is too static”;
- “focal hierarchy is weak”;
- style interpretation from ambiguous natural-language descriptors.

Learned advice must not override required constraints or silently mutate source.
## Constraint engine conclusions

Required properties of the solver:
- hard and soft constraints;
- explicit strength/priority;
- contradiction reporting;
- deterministic tie-breaking;
- incremental stability during editing;
- explainability/provenance: why a property ended up at a value;
- no silent required-constraint violation.

Relevant precedents included Cassowary/Auto Layout and linear/nonlinear optimization systems.

Recommended architecture:

```text
required geometry constraints
→ weighted linear/soft constraints
→ procedural placement / packing / collision passes
→ optional optical/perceptual objectives
→ advisory AI critique
```

The deterministic solver owns geometry. AI may propose or critique constraints but should not become the hidden authority that makes the same source render differently across runs.

## Authored state versus solved state

This is one of the most important research conclusions.

If an author says “the mascot is centered in the canvas,” resizing the canvas should change the **derived coordinate** without rewriting the authored relation into a new literal coordinate.

Therefore every important property may have:
- authored value/relationship;
- derived/solved value;
- provenance explaining the source;
- constraint provenance explaining the derivation.

This is necessary for stable responsive behavior and true source ↔ canvas round-trip editing.

## Language philosophy

The DSL should optimize for both humans and AI agents:
- compact;
- explicit;
- deterministic;
- diff-friendly;
- easy to generate correctly;
- resistant to malformed partial output;
- not excessively punctuation-heavy;
- not dependent on whitespace magic;
- canonical formatting available.

The research considered declarative, functional, Lisp-like, JSON/YAML-like, CSS-like, Swift/Rust-like and TypeScript-embedded forms. The recommended pattern is a **standalone concise declarative DSL backed by a canonical structured AST/GSIR**, not a syntax-only architecture.
## Type system and units

The language should be dimensionally safe. Candidate first-class types include:
- `Length`
- `Angle`
- `Color`
- `Opacity`
- `Point`
- `Vector`
- `Rect`
- `Path`
- `Shape`
- `Image`
- `Texture`
- `Font`
- `Object`
- `Region`
- `Constraint`
- `Percentage`
- `Depth`
- `Time`

Invalid operations such as `20px + 5deg` should fail at compile time. Responsive composition should use percentages, relations and constraints rather than forcing all values into one numeric scalar type.

## Styles, components and procedural design

Glyph should support reusable style definitions, components, variants, parameters, slots, overrides and design tokens, but avoid reproducing CSS's full complexity.

Procedural design should be deterministic by default. Randomness must be explicitly seeded so the same source produces the same result and diffs remain meaningful.
## Rendering architecture

Glyph should reuse mature rendering technology instead of creating a new raster engine.

Recommended structure:

```text
GSIR
→ constraint solve
→ backend-neutral resolved display list
→ target adapters
```

Target adapters may include:
- SVG;
- CommanderDraw;
- Skia/Core Graphics;
- Metal/WebGPU where justified;
- PDF;
- SwiftUI;
- Web/Canvas.

Pixel/vector-fidelity export and semantic code-generation export are different products. A complex illustration may sensibly export to SVG/PNG while a simple component may sensibly export to SwiftUI. Glyph should not claim every arbitrary illustration should become readable native UI code.

## Typography

Typography is not a decoration layer. It affects layout, hierarchy and visual rhythm and therefore belongs in the core scene model.

The research recommendation was to delegate shaping to mature engines such as CoreText/HarfBuzz rather than implement shaping from scratch. Glyph should own semantic text objects, layout constraints, typography intent and deterministic measurement contracts, while the shaping backend owns glyph shaping and font-system details.
## Image → Glyph / inverse graphics architecture

The research explicitly rejected a simplistic “vectorize every pixel” interpretation.

The goal is semantic reconstruction:

```text
image/reference
→ detect regions/text/objects
→ infer hierarchy and relationships
→ assign confidence + provenance
→ choose vector/raster/hybrid representation per region
→ construct editable GSIR
→ render
→ compare against reference
→ iteratively refine
```

Different source classes need different strategies:
- UI screenshots: OCR + rectangles + colors + typography + containment/layout relations;
- logos/icons: vector/path reconstruction;
- flat illustration: semantic regions + vector geometry + layered raster where needed;
- photography: usually hybrid raster/depth/masks, not fake “editable vectors” everywhere;
- painterly/generated art: preserve raster detail where semantic vectorization would destroy fidelity.

Confidence should exist independently for semantics, geometry and depth. Unknown depth should remain unknown. Inferred content must record where it came from and should be distinguishable from authored content.
## Round-trip editing and Glyph Studio

The target interaction model is:

```text
Glyph source ↔ canonical GSIR ↔ visual canvas/direct manipulation
```

The research looked to bidirectional transformations, projectional editors and lens-style thinking for the core requirement: direct manipulation must preserve semantic intent rather than flattening the document into new raw coordinates.

Recommended Studio structure:
- source pane;
- canvas/plate;
- semantic scene graph / ledger;
- inspector;
- constraint visualization;
- diff/change trace;
- optional reference image.

A canvas drag should answer “what authored intent should change?” If an object is centered by constraint, moving it may mean editing/removing the relation or creating an offset—not silently replacing the relation with a literal `x` coordinate.

Studio should remain a **semantic drafting bench**, not a Figma clone. Figma can be an import/export/interoperability surface only.

## Agent workflow

The canonical agent loop proposed by the research is:

```text
Agent queries scene
→ identifies semantic object by stable ID/role
→ inspects authored + derived state + provenance + constraints
→ applies one revision-safe semantic transaction
→ re-solves
→ computes semantic + visual diff
→ renders
→ visually critiques actual pixels
→ patches only the relevant object/relationship
```

This workflow is the core reason Glyph can be more reliable for agents than rewriting SVG or regenerating an image.
## Motion and interaction boundary

Motion should be a visual extension of the same semantic scene, not a separate animation file with anonymous layers.

Candidate motion concepts:
- timelines;
- named states;
- transitions;
- interpolation curves;
- springs;
- causal animation triggered by semantic state changes;
- object/relationship-level animation rather than path-index animation.

The research recommendation was to keep interaction bounded. Glyph should describe visual state and transitions; host applications should own general app logic, networking, business rules and navigation.

## Diagnostics, diffs and design tests

Agent-native diagnostics should include:
- `scene.objects` / role queries;
- constraints affecting an object;
- authored vs derived values;
- resolved bounds;
- collisions/overflow;
- palette and contrast;
- provenance/confidence;
- semantic diff;
- geometry diff;
- perceptual/pixel diff;
- “why is this here?” explanations.

Design assertions are useful for measurable conditions such as minimum contrast, no overlap, object count, containment or responsive bounds. They must not pretend that aesthetic quality is fully reducible to assertions.
## Accessibility

Accessibility should be represented in the semantic scene rather than bolted onto final pixels.

Research direction:
- semantic labels/roles;
- reading order;
- contrast assertions;
- minimum interactive target geometry where relevant;
- relationships between visible and assistive labels;
- export mappings to platform accessibility systems.

## Git, canonical formatting and mergeability

Glyph should be source-control friendly:
- stable IDs;
- deterministic serialization;
- canonical formatter;
- semantic/AST-aware diffs;
- conflict-aware merges;
- asset hashing;
- reproducible generated outputs.

Stable IDs are especially important because they let agents, diffs, visual editors and code review discuss the same object across revisions.

## Modules, packages and assets

A module/package system is necessary eventually, but the research warned against rebuilding a giant package manager prematurely.

Recommended progression:
1. local project imports;
2. reusable component/style modules;
3. explicit asset manifests + content hashes;
4. deterministic project bundles;
5. only then a broader package/distribution layer if real usage requires it.
## Performance, debugging and tooling

The research required Glyph to be practical for iterative agent use, not just theoretically expressive.

Important tooling requirements:
- fast incremental parse/solve/render loops;
- deterministic hot reload;
- CLI for check/compile/query/inspect/patch/diff/render;
- LSP/autocomplete/diagnostics;
- source spans and provenance in errors;
- object/constraint inspection;
- reproducible profiling and benchmark suites;
- explicit failure states rather than silent fallback.

A long-lived editing session should eventually keep an incremental solver alive rather than rebuilding all solving state every time. The v1 architecture should preserve that future path even if the first implementation rebuilds per solve.

## Commander integration

Glyph was researched specifically as an upgrade path for Commander, not as an isolated language.

Commander should be able to:
- query by semantic role;
- inspect stable objects;
- ask why geometry was derived;
- apply revision-safe transactions;
- compare semantic and resolved visual diffs;
- render through deterministic backends;
- inspect actual pixels;
- import bounded visual references into editable semantic scenes;
- use the same source/IR that Studio edits.

The intended high-level API is semantic and transactional, not “execute arbitrary source rewrite.”
## CommanderDraw reuse decision

The research compared four possibilities:
1. Glyph as a semantic/constraint layer above CommanderDraw;
2. Glyph replacing CommanderDraw's core;
3. a new portable core with `.draw.json` compatibility;
4. another hybrid architecture.

Conclusion:
- Keep **Glyph canonical** at the semantic/constraint level.
- Reuse CommanderDraw where it is already strong: deterministic vector rendering, editable geometry lowering, PNG/export paths, stable-node compatibility and visual QA.
- Do **not** make `.draw.json` Glyph's canonical data model.
- Preserve an adapter boundary so Glyph can target CommanderDraw now and additional renderers later.

This avoids throwing away useful rendering infrastructure while preventing CommanderDraw's lower-level node schema from constraining Glyph's semantic model.

## StepIsland benchmark requirement

StepIsland was chosen as a practical visual benchmark because it exposed the original weakness: technically valid code/vector drawings could still be visually simplistic and difficult to revise semantically.

Required comparison cases included:
1. icon;
2. mascot;
3. simple landscape;
4. complex Chinese-inspired landscape;
5. reproduce a reference;
6. modify one object without collateral change;
7. adapt aspect ratio;
8. create reusable iOS assets;
9. animate the environment;
10. generated illustration → editable representation.

Representations to compare: raw SVG, SwiftUI, current CommanderDraw and Glyph.
## Validation philosophy

The research explicitly required failure experiments, not only demonstrations.

Glyph should be considered valuable only if it measurably improves:
- semantic inspectability;
- edit locality;
- preservation of authored intent;
- responsive adaptation;
- deterministic reproduction;
- failure safety;
- cross-target identity;
- agent correction loops.

A validator score is never sufficient proof of visual quality. Representative renders must be inspected at actual target size. Reference-fidelity claims require pixel/visual review, not just schema validity.

The fixed effectiveness gate therefore included artifact breadth, role queries, surgical edits, hard-constraint integrity, responsive behavior, determinism, failure safety, stress/performance, bridge identity and manual visual sanity.

## Security and trust model

Important principles:
- deterministic source/IR is authoritative after compilation;
- inferred content carries provenance/confidence;
- external/generated assets should be hashed and cached/frozen when reproducibility matters;
- mutation APIs should be revision-safe and transactional;
- source edits should not silently rewrite unrelated design state;
- compiler/runtime failures should fail closed rather than emit apparently valid corrupted output.
## Recommended phased roadmap from the research

### Phase A — prove the semantic core
- canonical GSIR;
- stable IDs/roles;
- authored vs derived state;
- provenance;
- basic constraints;
- deterministic SVG/PNG path;
- transactional patch API;
- semantic diff/query/why;
- benchmark against existing approaches.

### Phase B — make the core genuinely reusable
- Cassowary-class weighted solver;
- components/styles/parameters;
- deterministic procedural generation;
- source round-trip editing;
- display list/object map;
- assertions, states and basic motion;
- formal StepIsland comparison.

### Phase C — native Studio + bounded inverse graphics
- source/canvas/scene-graph workflow;
- direct semantic selection and inspector editing;
- semantic visual diff;
- bounded screenshot/UI reconstruction with confidence/provenance;
- actual visual comparison and surgical edit proof.

### Phase D — language/GSIR v1 freeze
- dimensional types and units;
- schema/versioning/migrations;
- component slots/variants/typed parameters;
- modules/imports;
- richer masks/images/material/effect primitives.

### Phase E — production rendering and typography
- CoreText/HarfBuzz-class shaping;
- variable fonts/fallback/ligatures/kerning;
- renderer conformance;
- PDF/SwiftUI/Web/Canvas target strategy.

### Phase F — full Studio, broader inverse graphics and product hardening
- direct manipulation/constraints/undo-redo;
- hybrid photo/illustration reconstruction;
- motion runtime/timeline;
- accessibility;
- LSP/tooling;
- project/package/asset system;
- fuzzing/performance/security/release packaging.
## Risk register

Major risks identified by the research:

1. **Syntax-first failure** — building a pretty language without a meaningful semantic IR.
2. **Renderer reinvention** — wasting effort rebuilding Skia/Core Graphics instead of reusing mature engines.
3. **Ontology lock-in** — encoding model-specific categories as compiler types.
4. **Aesthetic overclaiming** — pretending deterministic critique or metrics equal taste.
5. **Round-trip collapse** — direct manipulation flattening constraints into literal coordinates.
6. **Nondeterminism** — equal-cost solver choices, unseeded randomness or neural outputs causing unstable diffs.
7. **Typography fidelity** — estimated metrics creating visually wrong text while tests still pass.
8. **Inverse-graphics overreach** — claiming arbitrary photos/illustrations can become clean semantic vectors.
9. **Package-system sprawl** — expanding ecosystem complexity before real reuse needs are proven.
10. **Editor sprawl** — drifting into a Figma clone instead of a semantic drafting environment.
11. **Agent safety** — blind source rewriting or stale transactions corrupting unrelated intent.
12. **Performance cliffs** — registering or solving unnecessary geometry at large scene sizes.

Mitigation theme: keep deterministic authority small and explicit, keep learned assistance advisory/confidence-bearing, and gate expansions with unchanged benchmarks.

## Competitive positioning

Glyph is not trying to beat every tool at its strongest dimension.

- SVG wins as a universal vector interchange/render format.
- SwiftUI/Compose/Flutter win for application UI composition in their ecosystems.
- Figma/Penpot win as mature collaborative visual editors.
- Houdini/Grasshopper win for deep procedural/node workflows.
- image generators win at rapid unconstrained visual synthesis.

Glyph's differentiated target is the intersection of:
- semantic visual intent;
- programmable/diffable source;
- constraints and responsive relationships;
- stable object identity;
- agent-native surgical edits;
- deterministic multi-target lowering;
- source ↔ canvas round-trip editing;
- bounded reconstruction from existing visuals.
## Naming and IP considerations

Research direction:
- “Glyph” is conceptually strong because it suggests a visual unit with structure and meaning.
- Before any public commercial launch, naming/trademark/domain/package availability should be checked formally.
- The product should avoid positioning that implies ownership of generic concepts such as vector graphics, constraint solving or scene graphs.
- Competitive differentiation should be described through the specific architecture and workflows, not claims that Glyph invented visual programming or parametric graphics.

## Open questions that remain legitimate

1. How much of the semantic model should be standardized versus free-form roles?
2. Which visual relationships deserve language primitives versus library functions or heuristics?
3. What is the smallest dimensional type system that gives real safety without making the DSL heavy?
4. How should direct manipulation choose among changing a literal, adding an offset, or changing/removing a constraint?
5. How should component instance overrides and source provenance interact across imported modules?
6. What subset of effects/materials should be canonical versus renderer-specific extensions?
7. Which export targets should promise semantic native code versus fidelity-only raster/vector output?
8. How should confidence and human acceptance promote inferred Image→Glyph state into authored state?
9. What is the right boundary between deterministic perceptual heuristics and optional learned critique?
10. When should motion remain in core Glyph versus a `Glyph Motion` extension?

These are product/architecture questions to resolve with implementation evidence, not reasons to abandon the semantic-IR direction.
## Final research decision: what to build first

The research's final recommendation was:

### Build first

1. Canonical semantic GSIR.
2. Stable IDs, open semantic roles and provenance.
3. Authored-versus-derived state.
4. Hard/soft relational constraints with deterministic solving.
5. Transactional patch APIs and stale-revision protection.
6. Query/inspect/why/diff diagnostics.
7. Backend-neutral resolved representation.
8. SVG/CommanderDraw-class rendering reuse.
9. Fixed benchmarks proving edit locality, determinism and responsive intent.

### Reuse from CommanderDraw

- existing deterministic vector/render/export stack;
- SVG-class geometry;
- CommanderDraw compatibility and production exports;
- stable-node editing concepts;
- native preview lessons;
- construction critique/reference comparison as supporting QA, never as semantic authority.

### Deliberately postpone until the core proves itself

- arbitrary Image→Glyph/photo reconstruction;
- broad neural geometry generation;
- a full Figma-like editor;
- complex motion/timeline system;
- huge semantic ontology;
- public package ecosystem;
- custom raster engine.

That sequencing was chosen to prevent the project from spending months on impressive demos before proving the semantic editing advantage.
## Implementation delta after the research

The project has now progressed substantially beyond the original “build first” recommendation.

Verified implementation as of the handoff:
- Rust Glyph core and `.glyph` DSL;
- canonical GSIR with stable IDs/roles/provenance/confidence;
- authored/derived separation;
- Cassowary-class weighted linear solver v1;
- revision-safe semantic transactions;
- semantic diff and resolved visual diff;
- query/inspect/why APIs;
- source property round-trip editing;
- styles/components/instances/seeded repeat;
- assertions, named states and sampled motion v1;
- backend-neutral display list/object map;
- SVG + CommanderDraw lowering/PNG;
- native macOS Glyph Studio v0.1;
- rendered-plate semantic selection;
- inspector editing and `LAST CHANGE` trace;
- bounded Apple Vision image import for structured UI;
- Commander plugin integration with 19 Glyph tools.

The fixed effectiveness gate passed before expansion and has remained green through the major architecture changes.

The central research hypothesis is therefore no longer theoretical: semantic stable-ID editing, constraint-preserving responsive behavior, deterministic lowering and low-collateral agent edits have been demonstrated in code and benchmarks.

What remains is mainly turning the validated architecture into a complete language/runtime/product: dimensional typing, richer graphics, real typography, direct-manipulation Studio, broader inverse graphics, motion, accessibility, modules/tooling and production hardening.
## Original research scope checklist preserved

The original research explicitly required investigation of all of the following areas:

1. Whether Glyph can be materially more controllable, composable, inspectable, reproducible and machine-understandable than SVG/Canvas/CSS/image generation.
2. Prior art across graphics, UI frameworks, design tools, creative coding, CAD/procedural systems, diagram languages, typography systems and inverse graphics.
3. Active falsification: determine whether a semantic IR over existing tools makes a new language unnecessary.
4. Semantic model: `Semantic → Composition → Constraint → Geometry → Rendering`.
5. Representation of semantics such as `tree` versus generic shape + semantic role.
6. First-class visual relationships: centered, left/right/above/below, crosses, behind, optical center, balance, negative space and hierarchy.
7. Classification of relations into formal constraints, heuristic optimization, learned suggestions and non-language concepts.
8. Constraint-engine research: Cassowary, linear/nonlinear optimization, packing/collision, saliency/optical heuristics, hard/soft priorities, contradictions, deterministic tie-breaks, incremental stability and provenance.
9. Style system: deterministic properties plus optional learned descriptors/profiles that are versioned/frozen.
10. Image→Glyph/photo/screenshot reconstruction with uncertainty and hybrid vector/raster/neural representation.
11. Deep typography integration with delegation to mature shaping engines.
12. Components, variants, parameters, slots, overrides and design tokens without CSS-level complexity.
13. Deterministic seeded procedural design.
14. Motion as core or extension: timelines, interpolation, springs, states and causal animation.
15. Interaction boundary so Glyph does not become a general-purpose app language.
16. Syntax options: declarative, functional, Lisp, JSON/YAML, CSS, Swift/Rust style, TypeScript embedded versus standalone.
17. Human-facing Glyph syntax + canonical AST/IR + binary/cache representation.
18. Dimensional type safety for lengths, angles, colors, points, paths, images, fonts, percentages, depth and time.
19. Responsive composition and aspect constraints.
20. Rendering backend comparison: Skia, Core Graphics, Metal, WebGPU, SVG DOM, Canvas, custom and hybrid.
21. Separation of pixel/vector fidelity exports from semantic code-generation exports.
22. Round-trip editing: `Glyph ↔ visual canvas ↔ direct manipulation` while preserving semantic intent.
23. Agent workflow: write/compile/render/critique/identify semantic object/patch/rerender/compare.
24. Diagnostics/APIs for objects, constraints, bounds, hierarchy, palette, contrast, collisions, render diff and provenance.
25. Semantic, geometry and perceptual visual diff.
26. Measurable design assertions without pretending aesthetics are fully numerical.
27. Accessibility integrated into scene representation.
28. Git-friendly canonical formatting, stable IDs and AST-aware diffs/merges.
29. Package/module model without reinventing npm.
30. Asset hashing/project bundles and frozen generated assets for reproducible renders.
31. Neural/deterministic split: deterministic authority for geometry/layout/constraints/typography placement/colors/masks/composition; neural assistance for decomposition, organic proposals, textures, semantic recognition, style interpretation and completion.
32. Performance, debugging, CLI, LSP and hot reload.
33. Editor concept: SOURCE | CANVAS | SCENE GRAPH + inspector/constraints; avoid a Figma clone.
34. Exact Commander integration APIs/workflow.
35. StepIsland benchmark against raw SVG, SwiftUI and CommanderDraw.
36. MVP, phased roadmap, concrete stack/security/competitive positioning/naming/failure experiments.
37. A complex illustrative example designed to expose architectural weaknesses, not merely show happy-path syntax.

## Original requested research deliverables

The deep research was expected to return:

A. Executive conclusion  
B. Prior-art landscape  
C. Opportunity analysis  
D. Product definition and non-goals  
E. Language philosophy  
F. Semantic scene model/schema  
G. Proposed syntax  
H. Compiler architecture  
I. Renderer architecture  
J. Constraint engine  
K. Image→Glyph architecture  
L. AI integration  
M. Commander integration  
N. Visual editor/round-trip  
O. Export strategy  
P. MVP  
Q. Roadmap  
R. Risk register  
S. Validation experiments  
T. Competitive analysis  
U. Naming/IP analysis  
V. Open questions

This archive preserves those substantive conclusions and the original scope even where the exact prose of the completed research response is not reproduced verbatim.
