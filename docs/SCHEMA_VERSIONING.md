# Glyph source and GSIR versioning

Glyph has three independent version numbers. They must never be conflated.

- **Source version** describes `.glyph` language syntax and semantics. Current: `1`.
- **GSIR schema version** describes serialized semantic scene structure. Current: `2`.
- **Scene revision** is user-edit transaction state. Migration never increments it.

A source file may declare `@version(1)` on the scene. Omission means source v1 for backward compatibility. An explicit version newer than the compiler is rejected rather than guessed or silently downgraded.

```glyph
scene "Example" @version(1) @id("example") {
  canvas 800px 600px;
}
```

## GSIR compatibility policy

Canonical writers emit only `CURRENT_GSIR_SCHEMA_VERSION`. Readers accept the current schema and explicitly implemented older migrations. A missing, zero, or future schema version fails closed.

The current supported migration is `GSIR v1 -> v2`. It is deterministic and one-way: it upgrades the schema marker, records source v1 when that field is absent, and leaves the scene revision unchanged.

A schema bump is required for an incompatible structural or semantic change. Adding an optional field with an unambiguous default may remain within the current schema. Any schema bump must ship with:

- a deterministic migration from every retained supported predecessor;
- regression tests proving migration preserves stable IDs and scene revision;
- a future-version rejection test;
- updated canonical serialization documentation.

## Migration invariants

Migrations operate on serialized GSIR before typed deserialization. They are pure with respect to external state and must be idempotent after reaching the current schema.

They must not silently:

- increment `revision`;
- regenerate stable node IDs;
- re-solve geometry as part of migration;
- coerce unsupported future syntax or schema into an older meaning;
- discard authored provenance, roles, confidence, constraints, or semantic metadata.

Solving happens after loading when a caller explicitly requires a resolved scene. Migration only makes the persisted semantic representation readable by the current runtime.
