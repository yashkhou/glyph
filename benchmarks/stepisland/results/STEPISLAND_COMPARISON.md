# Glyph StepIsland Cross-Representation Benchmark

This benchmark uses the same Sakura Lake scene across Glyph, raw semantic SVG, relational SwiftUI and CommanderDraw JSON. It reports measured structure and edit behavior; fewer literals are not treated as a visual-quality score.

## Representation size

| Representation | Lines | Bytes | Numeric literals |
|---|---:|---:|---:|
| glyph | 89 | 2003 | 85 |
| svg | 12 | 1941 | 121 |
| swiftui | 54 | 3016 | 99 |
| commander_draw | 153 | 3353 | 125 |

## Responsive intent test

Canvas width changes from 1290 to 1600 while title and mascot must remain horizontally centered.

| Representation | Source/runtime edits | Dependent center fields hand-edited | Result |
|---|---:|---:|---|
| Glyph | 1 transaction op | 0 | title=800.0, mascot=800.0, required violations=0 |
| SwiftUI | 1 source field | 0 | relational `centerX` preserves centers |
| raw SVG | 4 fields | 2 | constraints were lowered away |
| CommanderDraw | 3 fields | 2 | static interchange geometry |

## Surgical object edit

Changing only the right tree vertical radius is a one-location edit in every representation when the target is known. Glyph does not claim an advantage for this trivial case; its extra value is the typed semantic transaction/query/constraint layer around the edit.

- **glyph:** 1 changed field/line location(s).
- **svg:** 1 changed field/line location(s).
- **swiftui:** 1 changed field/line location(s).
- **commander_draw:** 1 changed field/line location(s).

## Semantic / relationship retention

- **glyph:** `{"machine_role_query": true, "relationship_declarations": 6, "role_declarations": 9, "stable_ids": 10}`
- **svg:** `{"machine_role_query": "DOM/XML query required; constraints absent", "relationship_declarations": 0, "role_attributes": 9, "stable_ids": 9}`
- **swiftui:** `{"machine_role_query": false, "relationship_variable_references": 9, "role_declarations": 0, "stable_ids": 8}`
- **commander_draw:** `{"machine_role_query": false, "relationship_declarations": 0, "role_declarations": 0, "stable_ids": 9}`

## Determinism

Five independent Glyph runs produced **1 unique GSIR hash** and **1 unique SVG hash**.

## Interpretation

- Raw SVG can retain excellent object IDs and semantic role attributes, but does not retain Glyph’s constraint graph after lowering.
- Well-authored SwiftUI can preserve responsive relationships with comparable source-edit locality; Glyph’s additional value is a graphics-specific semantic IR, renderer-neutral object map, revision-safe transactions, semantic queries and deterministic cross-target lowering.
- CommanderDraw remains a useful editable/rendering backend, while Glyph carries higher-level design intent above it.
- This benchmark supports continuing to Studio/Vision work, but does **not** justify replacing SwiftUI or SVG for every use case.
