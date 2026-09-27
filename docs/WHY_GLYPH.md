# Why Glyph

## AI design has a representation problem

Generative models are already capable of producing convincing interfaces, illustrations and visual assets. The weak point is often not generation. It is **continuity of intent after generation**.

A raster image can tell an agent what exists visually. It usually cannot tell the agent why an object is there, which relationship owns its position, which values were authored, which values were solved, which element is the canonical instance of a concept, or what can change without violating the design.

That is a representation problem.

Glyph is an attempt to build a representation specifically for the moment when software is no longer only rendering a design, but also **reasoning about it, editing it, explaining it and carrying it across targets**.

## Pixels are an output, not enough of an interface

Pixels are excellent final evidence. They are also intentionally lossy.

After rendering, many different design decisions collapse into the same visual result. Two elements can appear centered because of a constraint, because both coordinates were manually typed, because they are inside an auto-layout container, or by accident. A screenshot does not preserve that distinction.

For a human using a visual editor, some of that missing context is recoverable through convention and perception. For an autonomous agent making many edits, repeated reconstruction becomes a source of drift.

The system needs another layer.

## What that layer should preserve

Glyph is built around several pieces of information that are easy to lose in conventional render-first loops.

### Stable identity

An object should remain addressable across revisions. "The primary CTA" should not become "the rectangle near x=224" after one render.

Stable IDs make inspection, selection, transactions and diffs referential rather than spatial.

### Authored versus derived state

A resolved x-coordinate may be the output of a constraint rather than a value anyone intentionally chose.

That distinction matters. Editing a derived coordinate directly can fight the system that owns it. Glyph tracks authored and derived state so tooling can ask for the real owner of a value.

### Constraints as executable intent

Design relationships should be data, not a visual coincidence.

A required center alignment, a minimum gap or a weak preference can be solved, inspected and edited. That gives an agent a much smaller and safer action surface than rewriting arbitrary geometry.

### Provenance

Imported or inferred values should not become indistinguishable from hand-authored truth.

Glyph can retain source provenance and confidence so later tools can distinguish a known value from a reconstruction hypothesis.

### Semantic roles and accessibility

Meaning should be attached before the final render. If a node is a control, label, landmark or other semantic object, downstream systems should not have to infer that again from appearance.

### Deterministic diffs

An agent needs to know what changed because of its action.

Line diffs are useful for source. Pixel diffs are useful for appearance. Neither is sufficient alone for a semantic design system. Glyph also compares resolved objects by stable identity and reports movement, resize, geometry, style and content changes.

## Why not just use SVG?

SVG is an important Glyph output and a strong vector format. It is not the entire authoring/runtime contract Glyph is exploring.

A production SVG can preserve IDs and metadata, but it does not inherently provide Glyph's authored/derived distinction, revision-safe semantic transaction model, weighted source-level constraint graph, target-conformance contract, or source-authoritative inverse manipulation model.

Glyph can lower to SVG while retaining a richer upstream representation.

## Why not just generate SwiftUI, HTML or Canvas code?

Because a render target is not always a good canonical design model.

Target code carries target-specific concerns. A SwiftUI hierarchy and a Canvas draw loop can represent the same visual idea very differently. If the canonical design is one of those outputs, cross-target editing can become a translation problem dominated by implementation details.

Glyph keeps a backend-neutral semantic scene above those targets. Semantic code generation is bounded and fail-closed: it is used when the target can preserve the required meaning, not as a universal promise that any scene can become idiomatic application code automatically.

## Why fail closed?

AI systems are extremely good at producing something plausible when the exact answer is unavailable. That behavior is dangerous in infrastructure.

If a renderer or code generator cannot preserve a semantic requirement, a design tool should be able to say so explicitly. Silent degradation creates false confidence and makes later debugging much harder.

Glyph therefore treats conformance as data. A target can be exact, degraded or unsupported for a scene or feature, with machine-readable reasons.

## Why source-authoritative round trip?

A semantic visual language fails if visual editing immediately destroys its source model.

The current Glyph manipulation system therefore tries to invert supported direct-manipulation actions back into authored intent. A drag might safely change a free authored x/y value, adjust one equality constraint offset, or change an unambiguous component parameter. A resize can similarly be mapped back when ownership is clear.

When ownership is ambiguous, the runtime should stop rather than make a broad speculative rewrite.

That constraint is uncomfortable compared with a canvas that lets anything move anywhere. It is also what protects the design model from becoming fiction.

## Why deterministic artifacts?

Agent workflows need verification surfaces.

If compiling the same semantic state can produce unrelated identifiers, ordering or output noise, every downstream comparison becomes harder. Deterministic display lists, object maps, bundles and render artifacts reduce that noise and make automated verification practical.

Determinism does not guarantee visual quality. It makes quality and correctness easier to measure.

## The role of Glyph Studio

Glyph Studio is the visual workbench for this model, not the thesis itself.

The runtime should remain useful without a particular editor. Studio exists to test whether source, semantic state and direct manipulation can coexist in a human-usable loop: select an object, inspect what owns its geometry, make a supported edit, write it back to source, solve again and show the result.

The current v0.1 Studio is experimental and macOS-native. Its purpose is to pressure-test the round trip.

## Image reconstruction is useful — and bounded

A vision model can help bootstrap semantic structure from an existing interface image. Glyph includes a bounded Apple Vision-assisted path for UI-like reconstruction, with provenance/confidence retained where possible.

That does not make every photograph a clean latent design document. Arbitrary photographic reconstruction is a different problem and is outside the current contract.

Keeping that boundary explicit is part of the project philosophy.

## What exists today

Glyph v0.1.0 already contains working implementations of the core argument: a `.glyph` language, canonical GSIR, stable IDs, authored/derived state, provenance, weighted linear constraints, deterministic resolved outputs, semantic inspection, revision-safe transactions, semantic/visual diffing, source-backed edits, bounded direct manipulation, components, accessibility metadata, target conformance, project verification, language-service tooling, bounded semantic codegen and a native Studio workbench.

Those features are still experimental. The language and schemas are evolving, and several target subsets are intentionally narrow.

## The long-term bet

The current generation of AI design products mostly adds intelligent operators to file formats and canvases designed before intelligent operators existed.

Glyph asks a different question:

> If an agent were a first-class design collaborator from day one, what information would the design representation need to preserve?

The answer is unlikely to be "more pixels."

It is more likely to include stable identity, explicit relationships, semantic roles, authored intent, provenance, reversible operations, deterministic artifacts and target contracts that admit when they cannot preserve meaning.

That is the bet behind Glyph.

Pixel-first design files do not disappear. They become one layer in a larger stack — closer to an assembly representation than the source of truth an agent should reason over.

Glyph is an early attempt to build the layer above them.
