# Glyph Core Effectiveness Gate Report

## Verdict

**PASS** — the predeclared core gate is satisfied without changing its criteria after results were observed.

## Evidence

- 15 distinct curated artifact classes compile and render to SVG and PNG.
- 30 independent patch transactions produced exactly 0 unrelated authored-node mutations.
- Semantic role queries succeeded across all 15 curated scenes.
- Required conflicts and missing references now fail explicitly instead of returning false success.
- A 51-node dependency chain with 51 required constraints resolves fully.
- Six responsive scenes re-solve after material canvas-width changes without destructive authored-value rewrites.
- 20 repeated GSIR/SVG runs are byte-identical for representative scenes; repeated CommanderDraw PNG output is deterministic in the measured environment.
- CommanderDraw lowering preserves stable Glyph object IDs for all supported benchmark nodes.
- 100, 1,000 and 5,000-node synthetic scenes compile/check without crashes. Latest 5,000-node compile is about 1.3 s on this Mac.
- Manual contact-sheet inspection passed after correcting missing stroke rendering and clipped typography.

## Defects discovered by the gate

1. Parser accepted multiple property values in one statement. Fixed by making a property accept exactly one value.
2. `glyph check` returned exit code 0 for unresolved/required conflicts. Fixed.
3. The MVP solver stopped after eight passes and falsely reported success on deep dependency chains. Fixed with constraint-count-bounded convergence plus final required-constraint verification.
4. SVG and CommanderDraw lowering dropped stroke/opacity styling. Fixed and covered by tests.
5. Text had zero semantic bounds and no deterministic wrapping, causing visible clipping. Fixed with deterministic estimated text layout, multiline SVG tspans, CommanderDraw multiline lowering and semantic text bounds.

## Important limitation

The original gate was passed with estimated text metrics and the pre-Cassowary solver. Post-gate work has since replaced those two limitations on macOS: Glyph now uses a Cassowary-class weighted linear solver and CoreText shaping/line-breaking/metrics with explicit substitution/fallback diagnostics, measured letter spacing, ligature policy, and validated OpenType variable-font axes. Remaining typography limitations are cross-platform shaping parity and renderer-specific fidelity when a backend cannot reproduce CoreText's exact font cascade or advanced OpenType axis/feature selection.

## Gate artifacts

- `docs/BENCHMARK_PLAN.md` — criteria fixed before expansion.
- `benchmarks/run_core_gate.py` — repeatable machine gate.
- `benchmarks/results/core-gate.json` — machine-readable evidence.
- `benchmarks/results/core-gate-contact-sheet.png` — inspected render sheet.

With this gate passed, deferred roadmap implementation is authorized by the original plan.

## Post-gate expansion regression

The original gate has been rerun unchanged after each major expansion. It remains green after:

- replacement of the writer-based resolver with a Cassowary-class weighted linear solver;
- coupled-equation and soft-priority regression tests;
- source round-trip editing;
- reusable styles/components/seeded repeat, assertions, states and motion;
- backend-neutral display-list/object-map lowering used by SVG and CommanderDraw.

The first Cassowary integration temporarily regressed the 5,000-node synthetic case to ~8.5 s by registering unconstrained nodes. Restricting the linear graph to constraint-participating geometry restored the 5,000-node check/compile to roughly 1.2–1.3 s while preserving the stronger solver semantics.

## Native Studio + bounded image-import journey verification

The native Glyph Studio import journey has now been exercised against `benchmarks/generated/core-gate/15_ui_screen.png`, not only through CLI fixtures. Studio produced editable `15_ui_screen.glyph` plus its Vision manifest and opened the imported scene with a 9-object stable-ID ledger.

- `glyph check` passes the Studio-produced source: 9 nodes, 0 constraints, 0 conflicts, 0 unresolved relations.
- Rendering that exact source through the Commander `glyph_render_png` tool succeeds via CommanderDraw.
- Against the original 430×932 UI benchmark, the reconstructed render differs at 3,693 pixels (`0.9215%`) with mean channel delta `1.1567`. This is evidence for the bounded UI reconstruction case only, not arbitrary-image fidelity.
- A source-backed edit of `text_a592d7e0114f.content` from `Today` to `Tomorrow` yields exactly one authored semantic diff and exactly one changed visual object. The canvas and all unrelated stable objects remain unchanged.
- The before/after edit render differs at 2,211 pixels (`0.5517%`), localized to the heading region during visual inspection.
- A Commander plugin-runner failure exposed reliance on shell `PATH` when spawning Node for CommanderDraw. Glyph now resolves Node through `GLYPH_NODE_BIN` / `NODE_BINARY`, then `PATH`, then standard Homebrew/system locations. The repaired release binary renders successfully both through the Commander tool and under a deliberately restricted `PATH=/usr/bin:/bin`.
- Full Rust regression suite after the fix: 31 passed, 0 failed.

These results preserve the original architectural boundary: Apple Vision import is a bounded semantic reconstruction aid; deterministic Glyph source, stable identity, explicit diffs and CommanderDraw rendering remain the authority after import.
