# Glyph Effectiveness Benchmark — Predeclared Gate

This document fixes the pass/fail criteria **before** deferred roadmap features are expanded.

## Core hypothesis

Glyph is effective if semantic identity + relational intent measurably improve edit locality, inspectability, responsive adaptation and deterministic reproduction compared with treating visual work as anonymous low-level geometry.

## Pass gate

Core v0.x passes only if all mandatory gates below pass:

1. **Artifact breadth:** at least 12 distinct artifact classes compile and render to SVG; at least 8 render through the PNG bridge.
2. **Semantic inspectability:** every curated scene exposes stable IDs and meaningful roles; role queries find the intended objects without geometry search.
3. **Edit locality:** across at least 20 patch transactions, unrelated authored-node mutation must remain exactly 0.
4. **Constraint integrity:** required constraints must never be silently violated; conflicts must be surfaced explicitly.
5. **Responsive behavior:** at least 6 scenes must adapt to two materially different aspect ratios while preserving authored relational intent.
6. **Determinism:** 20 repeated compile/render runs of unchanged inputs must produce byte-identical GSIR and SVG.
7. **Failure safety:** malformed syntax, stale revisions, missing nodes and impossible required constraints must fail or report diagnostics without corrupting source/GSIR.
8. **Performance:** 100, 1,000 and 5,000-node synthetic scenes must compile/solve within documented bounds and without crashes on this Mac.
9. **Target integrity:** supported SVG/codegen targets must preserve stable semantic identity where the target contract allows it and fail closed otherwise.
10. **Visual sanity:** representative outputs must be visually inspected; no validator score may be treated as proof of design quality.
