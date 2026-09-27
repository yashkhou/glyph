# Typed components

Glyph source v1 supports reusable components with typed parameters, typed value slots, named variants, and explicit local overrides. Existing node-only components and instances remain valid.

```glyph
component button {
  param width: Length -> bg.width = 120px;
  param fill: Color -> bg.fill = "#223344";
  slot label: Text -> label.content = "Button";

  variant compact {
    set bg.width 96px;
    set bg.height 36px;
  }

  rect bg @id("bg") { x 0px; y 0px; width 120px; height 44px; rx 4px; fill "#223344"; }
  text label @id("label") { content "Button"; x 12px; y 28px; size 20px; fill "#ffffff"; }
}
```

```glyph
instance button primary @id("primary") {
  variant compact;
  param width 160px;
  slot label "Continue";
  override bg.rx 12px;
  x 40px;
  y 50px;
}
```

Component interface targets are local stable node IDs or names plus an authored property. Unknown targets, missing properties, duplicate interface names, duplicate variants, unknown instance assignments, and type mismatches fail at parse time.

A parameter or slot without a default is required. Scalar composite types that do not yet have literal syntax (`Point`, `Vector`, `Rect`, `Shape`, `Object`, `Region`, `Constraint`) are reserved in the type system but cannot yet be assigned as component literals.

## Precedence and identity

Instance materialization is deterministic. Authored values resolve in this order:

1. component template properties;
2. interface defaults;
3. the selected named variant;
4. explicit instance parameter and slot values;
5. explicit local `override` values;
6. legacy instance `x`, `y`, and `scale` transform.

Only one variant may be selected per instance in source v1. Generated stable IDs remain `<instance-id>/<template-id>`, so adding typed interfaces does not change existing constraint addressability or downstream object-map identity.
