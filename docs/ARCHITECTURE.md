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

## GSIR node model

Each addressable node contains:

- `id`: stable identity used by agents, diffs and render metadata;
- `kind`: small rendering-oriented structural kind (`rect`, `ellipse`, `path`, `text`, `group` in v0.1);
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

The IR already distinguishes `required`, `strong`, `medium`, and `weak` constraints. The v0.1 resolver supports equality and inequalities over common geometric properties and records relaxed/unresolved/conflicting relations.

**Important limitation:** v0.1 is not Cassowary. It is a deterministic priority-ordered resolver sufficient to validate the semantic architecture and benchmarks. The solver lives behind an isolated module so a Cassowary-class incremental solver can replace it without changing `.glyph`, GSIR, Commander tools, or patch transactions.

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

## Rendering

Glyph does not contain a custom rasterizer.

- SVG is emitted directly from the resolved scene and carries `id` and `data-glyph-role` metadata.
- PNG currently lowers the resolved scene to CommanderDraw `.draw.json` and invokes CommanderDraw's deterministic renderer.
- This bridge is compatibility/output only; `.draw.json` is not canonical Glyph state.

Longer-term, a backend-neutral display list can target Skia directly while keeping CommanderDraw support.

## Commander API

The v0.1 plugin exposes read and mutation operations separately:

- discover: `capabilities`, `query`, `inspect`, `why`;
- validate: `check`;
- mutate: `compile`, `apply`;
- compare: `diff`;
- render/export: SVG, PNG, CommanderDraw JSON.

Normal agent workflow:

```text
query role → inspect/why → apply transaction → solve → diff → render → visual critique
```

## Roadmap boundary

Next core work should be:

1. real incremental linear constraint solver;
2. components + parameterised reusable forms;
3. style/tokens + deterministic procedural generation;
4. stronger display-list/render metadata and object-ID maps;
5. formal StepIsland benchmark against SVG, SwiftUI and CommanderDraw.

Only after those pass should Glyph Studio, Vision import and Motion become primary implementation targets.
