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
- CommanderDraw bridge renders PNG.
- Benchmark patch modifies two fields on `tree_right`, with 0 unrelated authored nodes changed and 0 required violations.

## Test suite

`cargo test` currently covers parsing/solving, surgical editing, SVG semantic metadata, and responsive canvas re-solving.

## Not claimed yet

- The current resolver is not a full linear Cassowary implementation.
- Visual quality is not implied by compiler correctness.
- Image→Glyph semantic reconstruction is not implemented.
- No production round-trip canvas editor exists yet.
- Components, style inheritance/composition, procedural scatter and motion are roadmap work.

## Next go/no-go benchmark

Encode representative StepIsland assets and compare Glyph against raw SVG, SwiftUI and current CommanderDraw on:

- number of literal coordinates;
- edit locality;
- unintended authored changes;
- aspect-ratio adaptation;
- agent tool calls / failed edits;
- render reproducibility;
- semantic inspectability.

If Glyph does not materially reduce collateral edits and improve responsive/semantic manipulation, the abstraction should be reconsidered before Vision or Studio receives major investment.
