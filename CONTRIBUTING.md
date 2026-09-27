# Contributing to Glyph

Glyph is experimental systems software. Contributions are welcome, but changes should preserve the invariants that make the runtime useful to agents rather than only making one render look correct.

## Development setup

Requirements:

- Rust toolchain with Cargo
- macOS for the full native Studio / Apple Vision paths
- repository-local dependencies required by the existing scripts

Build and test the Rust core:

```bash
cargo build --release
cargo test
./scripts/verify.sh
```

Before opening a pull request, also run formatting and linting appropriate to the files you changed.

## Core invariants

Please treat these as compatibility constraints, not implementation details.

### Stable semantic identity

Do not casually regenerate object IDs, change their meaning across a transaction, or substitute spatial lookup where an existing stable ID is available.

### Authored state is not derived state

A solver result must not silently become authored intent. Keep the distinction explicit whenever new fields, edits or transforms are introduced.

### Determinism matters

Avoid unstable ordering, random identifiers, timestamps or environment-dependent output in canonical artifacts unless the format explicitly calls for them.

### Mutations must be explainable

Prefer the smallest source-backed or typed semantic operation. Revision-safe operations should continue to reject stale state.

### Fail closed on semantic loss

If a new renderer or codegen path cannot preserve required semantics, report that fact. Do not silently flatten or guess merely to return a successful-looking artifact.

### Preserve provenance

Imported/inferred data should retain enough provenance for downstream tools to distinguish reconstruction from authored fact.

## Where to start

Useful areas are separated in the repository:

- `src/` — parser, model, solver, runtime, diffs, source editing, targets and CLI
- `tests/` — core and robustness tests
- `studio/` — native macOS workbench
- `examples/` — authored Glyph scenes
- `docs/` — contracts and architecture
- `benchmarks/` — predeclared gates and comparison harnesses

Read `docs/ARCHITECTURE.md`, `docs/MVP.md`, and any subsystem-specific document before changing a public contract.

## Pull requests

A strong PR should state:

1. the behavior being changed;
2. which semantic contract or invariant it touches;
3. how the behavior is verified;
4. whether serialized source/GSIR/display-list output changes;
5. whether the change is backward compatible;
6. any target-conformance implications.

Add or update tests for parser, solver, edit, diff, conformance or rendering behavior when relevant.

If a change modifies a checked benchmark result, explain why the benchmark moved. Do not change gate criteria after seeing a result merely to make a gate pass.

## Scope discipline

Glyph intentionally has bounded subsystems. In particular, semantic SwiftUI/Web Canvas generation and image reconstruction should not be broadened by silently accepting unsupported behavior. Expand the contract only when the implementation and verification expand with it.

## License

By contributing, you agree that your contribution may be distributed under the repository's MIT License.
