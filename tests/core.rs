use glyph::{
    PatchOperation, Value, apply_transaction, diff::diff_scenes, parse_scene, render_svg,
    solve_scene,
};

const SCENE: &str = include_str!("../examples/sakura_lake.glyph");

#[test]
fn parses_and_solves_relational_scene() {
    let mut scene = parse_scene(SCENE, "test.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty());
    assert!(report.unresolved.is_empty());
    let bridge = scene.nodes.iter().find(|n| n.id == "bridge").unwrap();
    let lake = scene.nodes.iter().find(|n| n.id == "lake").unwrap();
    let bx = bridge.derived.props["x"] + bridge.derived.props["width"] / 2.0;
    let lx = lake.derived.props["cx"];
    assert!((bx - lx).abs() < 0.001);
}

#[test]
fn surgical_patch_changes_only_target_authored_node() {
    let mut scene = parse_scene(SCENE, "test.glyph").unwrap();
    solve_scene(&mut scene);
    let before = scene.clone();
    let result = apply_transaction(
        &mut scene,
        1,
        &[PatchOperation::Set {
            node: "tree_right".into(),
            path: "props.ry".into(),
            value: Value::Number(305.0),
        }],
    )
    .unwrap();
    assert_eq!(result.unrelated_authored_nodes_changed, 0);
    let diff = diff_scenes(&before, &scene);
    assert_eq!(diff.iter().filter(|d| d.category == "authored").count(), 1);
    assert_eq!(diff[0].node, "tree_right");
}

#[test]
fn svg_keeps_semantic_ids_and_roles() {
    let mut scene = parse_scene(SCENE, "test.glyph").unwrap();
    solve_scene(&mut scene);
    let svg = render_svg(&scene);
    assert!(svg.contains("id=\"tree_right\""));
    assert!(svg.contains("data-glyph-role=\"tree foreground framing\""));
}

#[test]
fn resizing_canvas_re_solves_centered_objects() {
    let mut scene = parse_scene(SCENE, "test.glyph").unwrap();
    solve_scene(&mut scene);
    let result = apply_transaction(
        &mut scene,
        1,
        &[PatchOperation::SetCanvas {
            width: Some(1600.0),
            height: None,
            background: None,
        }],
    )
    .unwrap();
    assert_eq!(result.required_constraints_violated, 0);
    let mascot = scene.nodes.iter().find(|n| n.id == "mascot").unwrap();
    let title = scene.nodes.iter().find(|n| n.id == "title").unwrap();
    assert_eq!(mascot.derived.props["cx"], 800.0);
    assert_eq!(title.derived.props["x"], 800.0);
}

#[test]
fn malformed_multi_value_property_is_rejected() {
    let bad = r#"scene "Bad" { canvas 100 100; rect a { x 1 y 2; } }"#;
    assert!(parse_scene(bad, "bad.glyph").is_err());
}

#[test]
fn required_constraint_conflicts_are_explicit() {
    let src = r##"scene "Conflict" {
        canvas 300 200;
        rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; }
        constrain a.center_x == 100 @required;
        constrain a.center_x == 200 @required;
    }"##;
    let mut scene = parse_scene(src, "conflict.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert_eq!(report.required_conflicts.len(), 1);
}

#[test]
fn missing_constraint_reference_is_unresolved() {
    let src = r##"scene "Missing" {
        canvas 300 200;
        rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; }
        constrain a.center_x == ghost.center_x @required;
    }"##;
    let mut scene = parse_scene(src, "missing.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert_eq!(report.unresolved, vec!["constraint.1"]);
}

#[test]
fn inequalities_move_geometry_only_when_needed() {
    let src = r##"scene "Ineq" {
        canvas 300 200;
        rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; }
        constrain a.left >= 100 @required;
    }"##;
    let mut scene = parse_scene(src, "ineq.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty());
    let a = scene.nodes.iter().find(|n| n.id == "a").unwrap();
    assert_eq!(a.derived.props["x"], 100.0);
}

#[test]
fn weaker_same_target_constraint_is_relaxed() {
    let src = r##"scene "Priority" {
        canvas 300 200;
        rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; }
        constrain a.center_x == 120 @strong;
        constrain a.center_x == 180 @weak;
    }"##;
    let mut scene = parse_scene(src, "priority.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.relaxed.contains(&"constraint.2".to_string()));
    let a = scene.nodes.iter().find(|n| n.id == "a").unwrap();
    assert_eq!(a.derived.props["x"], 95.0);
}

#[test]
fn svg_renders_stroke_and_opacity() {
    let src = r##"scene "Stroke" {
        canvas 200 120;
        path edge @id("edge") { d "M10 60 L190 60"; fill "none"; stroke "#123456"; stroke_width 7; opacity 0.5; }
    }"##;
    let mut scene = parse_scene(src, "stroke.glyph").unwrap();
    solve_scene(&mut scene);
    let svg = glyph::render_svg(&scene);
    assert!(svg.contains("stroke=\"#123456\""));
    assert!(svg.contains("stroke-width=\"7\""));
    assert!(svg.contains("opacity=\"0.5\""));
}

#[test]
fn commander_bridge_preserves_stroke_style_and_ids() {
    let src = r##"scene "Stroke" {
        canvas 200 120;
        path edge @id("stable_edge") { role "connector"; d "M10 60 L190 60"; fill "none"; stroke "#123456"; stroke_width 7; }
    }"##;
    let mut scene = parse_scene(src, "stroke.glyph").unwrap();
    solve_scene(&mut scene);
    let doc = glyph::commander_draw_document(&scene);
    assert_eq!(doc["nodes"][0]["id"], "stable_edge");
    assert_eq!(doc["nodes"][0]["style"]["stroke"], "#123456");
    assert_eq!(doc["nodes"][0]["style"]["strokeWidth"], 7.0);
}

#[test]
fn text_layout_wraps_and_has_bounds() {
    let src = r##"scene "Text" {
        canvas 500 300;
        text copy @id("copy") { role "copy"; content "Semantic design intent should wrap deterministically"; x 40; y 80; size 30; max_width 220; line_height 38; fill "#000"; }
    }"##;
    let mut scene = parse_scene(src, "text.glyph").unwrap();
    solve_scene(&mut scene);
    let node = &scene.nodes[0];
    let layout = glyph::layout_text_node(node);
    assert!(layout.lines.len() > 1);
    assert!(layout.width <= 220.0);
    assert!(layout.height > 38.0);
    let b = glyph::query::bounds(node).unwrap();
    assert!(b[2] > 0.0 && b[3] > 0.0);
}

#[test]
fn svg_and_commander_bridge_share_wrapped_lines() {
    let src = r##"scene "Text" { canvas 500 300; text copy @id("copy") { role "copy"; content "one two three four five six"; x 40; y 80; size 30; max_width 140; line_height 38; fill "#000"; } }"##;
    let mut scene = parse_scene(src, "text.glyph").unwrap();
    solve_scene(&mut scene);
    let layout = glyph::layout_text_node(&scene.nodes[0]);
    assert!(layout.lines.len() > 1);
    let svg = glyph::render_svg(&scene);
    assert!(svg.matches("<tspan").count() == layout.lines.len());
    let draw = glyph::commander_draw_document(&scene);
    assert!(draw["nodes"][0]["text"].as_str().unwrap().contains('\n'));
    assert_eq!(draw["nodes"][0]["lineHeight"], 38.0);
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_metrics_flow_through_solver_display_svg_and_bridge() {
    let src = r##"scene "CoreText" {
        canvas 500 220;
        text copy @id("copy") { content "office AV ffi"; x 32; y 90; size 32; font "Helvetica"; weight "700"; font_style "italic"; max_width 260; fill "#111"; }
    }"##;
    let mut scene = parse_scene(src, "coretext.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let node = &scene.nodes[0];
    let layout = glyph::layout_text_node(node);
    assert_eq!(layout.engine, "coretext");
    assert!(layout.width > 0.0 && layout.ascent > 0.0 && layout.descent > 0.0);
    assert!(layout.glyph_count > 0);
    assert!(!layout.resolved_font.is_empty());
    assert!(!layout.postscript_font.is_empty());
    assert!((node.derived.props["width"] - layout.width).abs() < 0.001);
    assert!((node.derived.props["ascent"] - layout.ascent).abs() < 0.001);
    assert_eq!(node.derived.diagnostics["text.engine"], "coretext");

    let display = glyph::build_display_list(&scene);
    assert_eq!(display.schema_version, 3);
    match &display.items[0].primitive {
        glyph::DisplayPrimitive::Text {
            shaping_engine,
            glyph_count,
            resolved_font,
            font_style,
            ..
        } => {
            assert_eq!(shaping_engine, "coretext");
            assert!(*glyph_count > 0);
            assert!(!resolved_font.is_empty());
            assert_eq!(font_style, "italic");
        }
        other => panic!("expected text primitive, got {other:?}"),
    }
    let svg = render_svg(&scene);
    assert!(svg.contains("data-glyph-text-engine=\"coretext\""), "{svg}");
    assert!(svg.contains("font-style=\"italic\""), "{svg}");
    let draw = glyph::commander_draw_document(&scene);
    assert_eq!(
        draw["nodes"][0]["glyphTextMetrics"]["shapingEngine"],
        "coretext"
    );
    assert_eq!(draw["nodes"][0]["fontStyle"], "italic");
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_reports_fallback_fonts_and_text_fit_overflow() {
    let src = r##"scene "Fallback" {
        canvas 300 180;
        text sample @id("sample") { content "A😀漢"; x 20; y 80; size 56; font "Helvetica"; max_width 1; fill "#000"; }
    }"##;
    let mut scene = parse_scene(src, "fallback.glyph").unwrap();
    solve_scene(&mut scene);
    let node = &scene.nodes[0];
    let layout = glyph::layout_text_node(node);
    assert_eq!(layout.engine, "coretext");
    assert!(
        layout.overflow,
        "single shaped cluster wider than max_width should report overflow"
    );
    assert_eq!(node.derived.diagnostics["text.fit"], "overflow");
    assert!(
        !layout.fallback_fonts.is_empty(),
        "CoreText should cascade Helvetica for emoji/CJK text: {layout:?}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_letter_spacing_and_ligature_policy_are_semantic() {
    let base_src = r##"scene "Base" { canvas 400 160; text sample @id("sample") { content "office ffi"; x 20; y 80; size 36; font "Helvetica"; fill "#000"; } }"##;
    let spaced_src = r##"scene "Spaced" { canvas 400 160; text sample @id("sample") { content "office ffi"; x 20; y 80; size 36; font "Helvetica"; letter_spacing 4px; ligatures "none"; fill "#000"; } }"##;
    let mut base = parse_scene(base_src, "base-text.glyph").unwrap();
    let mut spaced = parse_scene(spaced_src, "spaced-text.glyph").unwrap();
    solve_scene(&mut base);
    solve_scene(&mut spaced);
    let base_layout = glyph::layout_text_node(&base.nodes[0]);
    let spaced_layout = glyph::layout_text_node(&spaced.nodes[0]);
    assert!(
        spaced_layout.width > base_layout.width + 10.0,
        "base={base_layout:?} spaced={spaced_layout:?}"
    );
    assert_eq!(spaced_layout.letter_spacing, 4.0);
    assert_eq!(spaced_layout.ligatures, "none");
    assert_eq!(
        spaced.nodes[0].derived.diagnostics["text.letter_spacing"],
        "4"
    );
    assert_eq!(
        spaced.nodes[0].derived.diagnostics["text.ligatures"],
        "none"
    );
    let svg = render_svg(&spaced);
    assert!(svg.contains("letter-spacing=\"4\""), "{svg}");
    assert!(svg.contains("data-glyph-ligatures=\"none\""), "{svg}");
    let draw = glyph::commander_draw_document(&spaced);
    assert_eq!(draw["nodes"][0]["letterSpacing"], 4.0);
    assert_eq!(draw["nodes"][0]["glyphTextMetrics"]["ligatures"], "none");
}

#[cfg(target_os = "macos")]
#[test]
fn coretext_font_substitution_is_explicit_and_exported() {
    let src = r##"scene "Substitution" { canvas 400 160; text sample @id("sample") { content "Editorial"; x 20; y 80; size 40; font "New York"; weight "600"; fill "#000"; } }"##;
    let mut scene = parse_scene(src, "substitution.glyph").unwrap();
    solve_scene(&mut scene);
    let layout = glyph::layout_text_node(&scene.nodes[0]);
    assert!(layout.font_substituted, "{layout:?}");
    assert_ne!(layout.resolved_font, layout.requested_font);
    assert_eq!(
        scene.nodes[0].derived.diagnostics["text.font.substituted"],
        "true"
    );
    let svg = render_svg(&scene);
    assert!(
        svg.contains("data-glyph-font-substituted=\"true\""),
        "{svg}"
    );
    let draw = glyph::commander_draw_document(&scene);
    assert_eq!(
        draw["nodes"][0]["glyphTextMetrics"]["fontSubstituted"],
        true
    );
    assert!(
        draw["meta"]["bridgeWarnings"]
            .to_string()
            .contains("New York")
    );
}

#[cfg(target_os = "macos")]
#[test]
fn variable_font_width_axis_changes_coretext_geometry_and_exports() {
    let narrow_src = r##"scene "Variable Narrow" { canvas 500 180; text sample @id("sample") { content "Variable Width"; x 20; y 90; size 48; font "Skia"; font_axis_wdth 0.62; fill "#000"; } }"##;
    let wide_src = r##"scene "Variable Wide" { canvas 500 180; text sample @id("sample") { content "Variable Width"; x 20; y 90; size 48; font "Skia"; font_axis_wdth 1.3; fill "#000"; } }"##;
    let mut narrow = parse_scene(narrow_src, "variable-narrow.glyph").unwrap();
    let mut wide = parse_scene(wide_src, "variable-wide.glyph").unwrap();
    let narrow_report = solve_scene(&mut narrow);
    let wide_report = solve_scene(&mut wide);
    assert!(narrow_report.unresolved.is_empty(), "{narrow_report:?}");
    assert!(wide_report.unresolved.is_empty(), "{wide_report:?}");
    let narrow_layout = glyph::layout_text_node(&narrow.nodes[0]);
    let wide_layout = glyph::layout_text_node(&wide.nodes[0]);
    assert_eq!(narrow_layout.font_variations["wdth"], 0.62);
    assert_eq!(wide_layout.font_variations["wdth"], 1.3);
    assert!(
        wide_layout.width > narrow_layout.width + 5.0,
        "narrow={narrow_layout:?} wide={wide_layout:?}"
    );
    assert_eq!(
        wide.nodes[0].derived.diagnostics["text.font.variations"],
        "wdth=1.3"
    );
    let svg = render_svg(&wide);
    assert!(
        svg.contains("font-variation-settings=\"'wdth' 1.3\""),
        "{svg}"
    );
    assert!(
        svg.contains("data-glyph-font-variations=\"'wdth' 1.3\""),
        "{svg}"
    );
    let draw = glyph::commander_draw_document(&wide);
    assert_eq!(
        draw["nodes"][0]["glyphTextMetrics"]["fontVariations"]["wdth"],
        1.3
    );
    assert!(
        draw["meta"]["bridgeWarnings"]
            .to_string()
            .contains("variable-font axes")
    );
}

#[cfg(target_os = "macos")]
#[test]
fn variable_font_axes_fail_closed_when_unsupported_or_out_of_range() {
    for (source, needle) in [
        (
            r##"scene "Bad Axis" { canvas 200 100; text sample { content "x"; x 0; y 50; size 32; font "Skia"; font_axis_opsz 12; } }"##,
            "does not expose requested variation axis opsz",
        ),
        (
            r##"scene "Bad Range" { canvas 200 100; text sample { content "x"; x 0; y 50; size 32; font "Skia"; font_axis_wdth 9; } }"##,
            "outside supported range",
        ),
    ] {
        let mut scene = parse_scene(source, "bad-variable.glyph").unwrap();
        let report = solve_scene(&mut scene);
        assert!(
            report.unresolved.iter().any(|entry| entry.contains(needle)),
            "{report:?}"
        );
    }
    let malformed = r##"scene "Bad Tag" { canvas 200 100; text sample { content "x"; x 0; y 50; size 32; font "Skia"; font_axis_w 1; } }"##;
    let err = parse_scene(malformed, "bad-tag.glyph").unwrap_err();
    assert!(err.to_string().contains("exactly four ASCII"), "{err}");
}

#[test]
fn invalid_ligature_mode_is_rejected() {
    let src = r##"scene "Bad Ligatures" { canvas 100 100; text sample { content "ffi"; x 0; y 20; size 16; ligatures "mystery"; } }"##;
    let err = parse_scene(src, "bad-ligatures.glyph").unwrap_err();
    assert!(
        err.to_string().contains("expects none, standard, or all"),
        "{err}"
    );
}

#[test]
fn styles_components_repeat_motion_and_states_parse() {
    let src = r##"scene "Reuse" {
        canvas 600 400;
        style soft { fill "#ddeedd"; rx 12; }
        component chip { rect bg @id("bg") { use soft; role "chip"; x 0; y 0; width 100; height 44; } text label @id("label") { role "label"; content "Hi"; x 20; y 29; size 20; fill "#223322"; } }
        instance chip a @id("a") { x 40; y 50; role "first"; }
        rect dot @id("dot") { role "dot"; x 40; y 160; width 10; height 10; fill "#445566"; }
        repeat dot count 3 seed 7 { dx 30; jitter_y 4; }
        animate a/bg.x from 40 to 200 duration 0.5 easing "easeInOut";
        state active { set dot.fill "#ffffff"; }
        assert dot.left >= 0;
    }"##;
    let scene = parse_scene(src, "reuse.glyph").unwrap();
    assert_eq!(scene.styles.len(), 1);
    assert_eq!(scene.components.len(), 1);
    assert_eq!(scene.animations.len(), 1);
    assert_eq!(scene.states.len(), 1);
    assert_eq!(scene.assertions.len(), 1);
    assert!(scene.nodes.iter().any(|n| n.id == "a/bg"));
    assert_eq!(
        scene
            .nodes
            .iter()
            .filter(|n| n.roles.iter().any(|r| r == "generated"))
            .count(),
        3
    );
    let bg = scene.nodes.iter().find(|n| n.id == "a/bg").unwrap();
    assert_eq!(bg.authored.props["fill"].text(), Some("#ddeedd"));
}

#[test]
fn procedural_repeat_is_seed_deterministic() {
    let src = r##"scene "R" { canvas 300 200; rect dot @id("dot") { x 10; y 10; width 8; height 8; fill "#000"; } repeat dot count 5 seed 123 { dx 20; jitter_x 5; jitter_y 5; } }"##;
    let a = parse_scene(src, "r.glyph").unwrap();
    let b = parse_scene(src, "r.glyph").unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

#[test]
fn assertions_states_and_motion_execute() {
    let src = r##"scene "Runtime" {
        canvas 400 300;
        rect box @id("box") { x 20; y 30; width 100; height 80; fill "#111111"; }
        assert box.left >= 0;
        state active { set box.fill "#ff0000"; set box.x 60; }
        animate box.x from 20 to 220 duration 1 easing "linear";
    }"##;
    let mut scene = parse_scene(src, "runtime.glyph").unwrap();
    solve_scene(&mut scene);
    assert!(glyph::evaluate_assertions(&scene).iter().all(|r| r.passed));
    let active = glyph::scene_in_state(&scene, "active").unwrap();
    assert_eq!(
        active.nodes[0].authored.props["fill"].text(),
        Some("#ff0000")
    );
    assert_eq!(active.nodes[0].derived.props["x"], 60.0);
    let half = glyph::sample_motion(&scene, 0.5).unwrap();
    assert_eq!(half.nodes[0].derived.props["x"], 120.0);
}

#[test]
fn motion_timeline_supports_delay_repeat_alternate_and_units() {
    let src = r##"scene "Timeline" {
        canvas 400 200;
        rect box @id("box") { x 20px; y 30px; width 80px; height 40px; }
        animate box.x from 20px to 120px duration 500ms delay 250ms easing "linear" repeat 2 alternate;
    }"##;
    let scene = parse_scene(src, "timeline.glyph").unwrap();
    let animation = &scene.animations[0];
    assert_eq!(animation.delay, 0.25);
    assert_eq!(animation.delay_unit, Some(glyph::Unit::Ms));
    assert_eq!(animation.repeat, 2);
    assert!(animation.alternate);

    let before = glyph::sample_motion(&scene, 0.1).unwrap();
    assert_eq!(before.nodes[0].derived.props["x"], 20.0);
    let first_half = glyph::sample_motion(&scene, 0.5).unwrap();
    assert_eq!(first_half.nodes[0].derived.props["x"], 70.0);
    let turn = glyph::sample_motion(&scene, 0.75).unwrap();
    assert_eq!(turn.nodes[0].derived.props["x"], 120.0);
    let reverse_half = glyph::sample_motion(&scene, 1.0).unwrap();
    assert_eq!(reverse_half.nodes[0].derived.props["x"], 70.0);
    let end = glyph::sample_motion(&scene, 1.25).unwrap();
    assert_eq!(end.nodes[0].derived.props["x"], 20.0);
}

#[test]
fn motion_springs_are_deterministic_and_invalid_options_fail_closed() {
    let src = r##"scene "Spring" {
        canvas 300 160;
        rect box @id("box") { x 10; y 20; width 50; height 40; }
        animate box.x from 10 to 110 duration 1 easing "springSnappy";
    }"##;
    let scene = parse_scene(src, "spring.glyph").unwrap();
    let first = glyph::sample_motion(&scene, 0.42).unwrap();
    let second = glyph::sample_motion(&scene, 0.42).unwrap();
    assert_eq!(
        first.nodes[0].derived.props["x"].to_bits(),
        second.nodes[0].derived.props["x"].to_bits()
    );
    assert_eq!(
        glyph::sample_motion(&scene, 0.0).unwrap().nodes[0]
            .derived
            .props["x"],
        10.0
    );
    assert_eq!(
        glyph::sample_motion(&scene, 1.0).unwrap().nodes[0]
            .derived
            .props["x"],
        110.0
    );
    assert!(
        glyph::sample_motion(&scene, -0.1)
            .unwrap_err()
            .to_string()
            .contains("nonnegative")
    );

    for (source, needle) in [
        (
            r#"scene "Bad" { canvas 100 100; rect x { x 0; y 0; width 10; height 10; } animate x.x from 0 to 10 duration 1 repeat 0; }"#,
            "positive integer",
        ),
        (
            r#"scene "Bad" { canvas 100 100; rect x { x 0; y 0; width 10; height 10; } animate x.x from 0 to 10 duration 1 delay -1s; }"#,
            "delay must be nonnegative",
        ),
        (
            r#"scene "Bad" { canvas 100 100; rect x { x 0; y 0; width 10; height 10; } animate x.x from 0 to 10 duration 1 easing "mystery"; }"#,
            "unsupported animation easing",
        ),
    ] {
        let err = parse_scene(source, "bad-motion.glyph").unwrap_err();
        assert!(err.to_string().contains(needle), "{err}");
    }
}

#[test]
fn component_instance_ids_are_constraint_addressable() {
    let src = r##"scene "C" { canvas 400 300; component dot { ellipse e @id("e") { cx 0; cy 0; rx 10; ry 10; fill "#000"; } } instance dot first @id("first") { x 50; y 50; } constrain first/e.center_x == canvas.center_x @required; }"##;
    let mut scene = parse_scene(src, "c.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty());
    let n = scene.nodes.iter().find(|n| n.id == "first/e").unwrap();
    assert_eq!(n.derived.props["cx"], 200.0);
}

#[test]
fn source_edit_changes_only_requested_value() {
    let src = "scene \"Roundtrip\" {\n  canvas 400 300;\n  rect box @id(\"box\") {\n    role \"surface\";\n    x 20;\n    y 30;\n    width 100;\n    height 80;\n    fill \"#111111\";\n  }\n  constrain box.center_x == canvas.center_x @strong;\n}\n";
    let edited =
        glyph::set_source_property(src, "box", "width", glyph::Value::Number(144.0)).unwrap();
    assert_eq!(edited.matches("width 144;").count(), 1);
    assert_eq!(src.replace("width 100;", "width 144;"), edited);
    let scene = parse_scene(&edited, "edited.glyph").unwrap();
    assert_eq!(scene.nodes[0].authored.props["width"].number(), Some(144.0));
}

#[test]
fn source_edit_can_insert_missing_property_without_reformatting_document() {
    let src = "scene \"Insert\" {\n  canvas 200 200;\n  rect a @id(\"a\") { x 10; y 10; width 20; height 20; }\n}\n";
    let edited =
        glyph::set_source_property(src, "a", "fill", glyph::Value::Text("#abc".into())).unwrap();
    assert!(edited.contains("fill \"#abc\";"));
    assert!(edited.contains("canvas 200 200;"));
}

#[test]
fn linear_solver_resolves_coupled_equal_width_system() {
    let src = r##"scene "Coupled" {
  canvas 300 100;
  rect a @id("a") { x 0; y 0; width 100; height 20; }
  rect b @id("b") { x 160; y 0; width 100; height 20; }
  constrain a.left == 0 @required;
  constrain b.right == canvas.right @required;
  constrain b.left == a.right + 10 @required;
  constrain a.width == b.width @required;
}"##;
    let mut scene = parse_scene(src, "coupled.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let a = scene.nodes.iter().find(|n| n.id == "a").unwrap();
    let b = scene.nodes.iter().find(|n| n.id == "b").unwrap();
    assert!((a.derived.props["width"] - 145.0).abs() < 0.001);
    assert!((b.derived.props["width"] - 145.0).abs() < 0.001);
    assert!((b.derived.props["x"] - 155.0).abs() < 0.001);
}

#[test]
fn linear_solver_honors_soft_strength_priority() {
    let src = r##"scene "Priority" {
  canvas 300 100;
  rect box @id("box") { x 0; y 0; width 100; height 20; }
  constrain box.left == 0 @required;
  constrain box.right == canvas.right @strong;
  constrain box.width == 120 @weak;
}"##;
    let mut scene = parse_scene(src, "priority.glyph").unwrap();
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    assert!(
        report.relaxed.contains(&"constraint.3".to_string()),
        "{report:?}"
    );
    let box_node = scene.nodes.iter().find(|n| n.id == "box").unwrap();
    assert!((box_node.derived.props["width"] - 300.0).abs() < 0.001);
}

#[test]
fn display_list_preserves_order_bounds_and_provenance() {
    let mut scene = parse_scene(SCENE, "display.glyph").unwrap();
    solve_scene(&mut scene);
    let display = glyph::build_display_list(&scene);
    assert_eq!(display.items.len(), scene.nodes.len());
    assert_eq!(
        display.items.iter().map(|i| &i.id).collect::<Vec<_>>(),
        scene.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    let lake = display.object_map.get("lake").unwrap();
    assert_eq!(
        lake.index,
        scene.nodes.iter().position(|n| n.id == "lake").unwrap()
    );
    assert!(lake.bounds.is_some());
    assert_eq!(lake.provenance.source, "display.glyph");
}

#[test]
fn display_list_keeps_wrapped_text_and_is_deterministic() {
    let src = r##"scene "Text DL" { canvas 220 160; text label @id("label") { role "copy"; x 20; y 40; content "A deliberately long line that must wrap predictably"; size 18; max_width 120; fill "#111"; } }"##;
    let mut scene = parse_scene(src, "text-dl.glyph").unwrap();
    solve_scene(&mut scene);
    let a = glyph::build_display_list(&scene);
    let b = glyph::build_display_list(&scene);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    match &a.items[0].primitive {
        glyph::DisplayPrimitive::Text { lines, .. } => assert!(lines.len() > 1),
        other => panic!("expected text display primitive, got {other:?}"),
    }
    assert!(a.object_map["label"].bounds.is_some());
}

#[test]
fn visual_diff_classifies_resize_without_false_move() {
    let mut before = parse_scene(SCENE, "visual-before.glyph").unwrap();
    solve_scene(&mut before);
    let mut after = before.clone();
    apply_transaction(
        &mut after,
        1,
        &[PatchOperation::Set {
            node: "tree_right".into(),
            path: "props.ry".into(),
            value: Value::Number(305.0),
        }],
    )
    .unwrap();
    let report = glyph::diff::visual_diff(&before, &after);
    assert_eq!(report.changed_objects.len(), 1);
    let change = &report.changed_objects[0];
    assert_eq!(change.id, "tree_right");
    assert!(change.resized);
    assert!(!change.moved);
    assert!(change.geometry_changed);
    assert!(!change.style_changed);
    assert!(change.changed_fields.contains(&"size".to_string()));
    assert!(!change.changed_fields.contains(&"position".to_string()));
}

#[test]
fn visual_diff_reports_constraint_driven_canvas_motion() {
    let mut before = parse_scene(SCENE, "visual-wide-before.glyph").unwrap();
    solve_scene(&mut before);
    let mut after = before.clone();
    apply_transaction(
        &mut after,
        1,
        &[PatchOperation::SetCanvas {
            width: Some(1600.0),
            height: None,
            background: None,
        }],
    )
    .unwrap();
    let report = glyph::diff::visual_diff(&before, &after);
    assert!(report.canvas_changed);
    for id in ["mascot", "title"] {
        let change = report.changed_objects.iter().find(|c| c.id == id).unwrap();
        assert!(change.moved, "{id} should move with the canvas constraint");
        assert!(!change.resized, "{id} should not resize");
    }
}

#[test]
fn visual_diff_reports_style_only_edits() {
    let mut before = parse_scene(SCENE, "visual-style-before.glyph").unwrap();
    solve_scene(&mut before);
    let mut after = before.clone();
    apply_transaction(
        &mut after,
        1,
        &[PatchOperation::Set {
            node: "tree_right".into(),
            path: "props.fill".into(),
            value: Value::Text("#ff00aa".into()),
        }],
    )
    .unwrap();
    let report = glyph::diff::visual_diff(&before, &after);
    assert_eq!(report.changed_objects.len(), 1);
    let change = &report.changed_objects[0];
    assert_eq!(change.id, "tree_right");
    assert!(change.style_changed);
    assert!(!change.geometry_changed);
    assert!(!change.moved);
    assert!(!change.resized);
    assert_eq!(change.changed_fields, vec!["style"]);
}

#[test]
fn metadata_and_confidence_survive_source_parse() {
    let src = r##"scene "Vision" @id("vision_scene") {
  canvas 320 200 background "#ffffff";
  meta "vision.source" "/tmp/reference.png";
  meta "vision.importer" "apple-vision-v1";
  rect region @id("region_001") {
    role "inferred" "region";
    confidence semantics 0.82 geometry 0.97 depth unknown;
    x 10; y 20; width 80; height 40; fill "#eeeeee";
  }
}"##;
    let scene = parse_scene(src, "vision.glyph").unwrap();
    assert_eq!(scene.metadata["vision.source"], "/tmp/reference.png");
    assert_eq!(scene.metadata["vision.importer"], "apple-vision-v1");
    let confidence = scene.nodes[0].confidence.as_ref().unwrap();
    assert_eq!(confidence.semantics, Some(0.82));
    assert_eq!(confidence.geometry, Some(0.97));
    assert_eq!(confidence.depth, None);
}

#[test]
fn confidence_outside_unit_interval_is_rejected() {
    let src = r#"scene "Bad Confidence" {
  canvas 100 100;
  rect region {
    confidence semantics 1.2 geometry 0.5 depth unknown;
    x 0; y 0; width 10; height 10;
  }
}"#;
    let err = parse_scene(src, "bad-confidence.glyph").unwrap_err();
    assert!(
        err.to_string()
            .contains("confidence value must be within [0, 1]")
    );
}

#[test]
fn vision_manifest_generates_deterministic_editable_source() {
    use glyph::vision::{VisionBox, VisionManifest, VisionObservation, manifest_to_glyph};
    let manifest = VisionManifest {
        schema_version: 1,
        source: "/tmp/Vision Sample.png".into(),
        width: 640,
        height: 360,
        background: Some("#f7f5ef".into()),
        observations: vec![
            VisionObservation {
                kind: "rectangle".into(),
                text: None,
                bbox: VisionBox {
                    x: 40.0,
                    y: 90.0,
                    width: 240.0,
                    height: 120.0,
                },
                confidence: 0.91,
                color: Some("#ffffff".into()),
                corner_radius: Some(24.0),
                font_size: None,
                font_weight: None,
                baseline_y: None,
            },
            VisionObservation {
                kind: "text".into(),
                text: Some("Editable title".into()),
                bbox: VisionBox {
                    x: 56.0,
                    y: 42.0,
                    width: 180.0,
                    height: 28.0,
                },
                confidence: 0.97,
                color: Some("#111111".into()),
                corner_radius: None,
                font_size: Some(31.5),
                font_weight: Some("600".into()),
                baseline_y: Some(64.0),
            },
        ],
    };
    let a = manifest_to_glyph(&manifest).unwrap();
    let b = manifest_to_glyph(&manifest).unwrap();
    assert_eq!(a, b);
    let scene = parse_scene(&a, "vision-generated.glyph").unwrap();
    assert_eq!(scene.metadata["vision.importer"], "apple-vision-v1");
    assert_eq!(scene.nodes.len(), 2);
    assert!(scene.nodes.iter().all(|node| node.confidence.is_some()));
    let text = scene
        .nodes
        .iter()
        .find(|node| node.roles.iter().any(|r| r == "text"))
        .unwrap();
    assert!(text.id.starts_with("text_"));
    assert_eq!(text.authored.props["size"].number(), Some(31.5));
    assert_eq!(text.authored.props["y"].number(), Some(64.0));
    assert_eq!(text.authored.props["weight"].text(), Some("600"));
    assert!(!text.authored.props.contains_key("max_width"));
    let edited =
        glyph::set_source_property(&a, &text.id, "content", Value::Text("Changed title".into()))
            .unwrap();
    let reparsed = parse_scene(&edited, "vision-edited.glyph").unwrap();
    let changed = reparsed
        .nodes
        .iter()
        .find(|node| node.id == text.id)
        .unwrap();
    assert_eq!(
        changed.authored.props["content"].text(),
        Some("Changed title")
    );
}

#[test]
fn vision_manifest_rejects_bbox_outside_canvas() {
    use glyph::vision::{VisionBox, VisionManifest, VisionObservation, manifest_to_glyph};
    let manifest = VisionManifest {
        schema_version: 1,
        source: "/tmp/bad.png".into(),
        width: 100,
        height: 100,
        background: Some("#ffffff".into()),
        observations: vec![VisionObservation {
            kind: "rectangle".into(),
            text: None,
            bbox: VisionBox {
                x: 80.0,
                y: 10.0,
                width: 30.0,
                height: 20.0,
            },
            confidence: 0.9,
            color: None,
            corner_radius: None,
            font_size: None,
            font_weight: None,
            baseline_y: None,
        }],
    };
    let err = manifest_to_glyph(&manifest).unwrap_err();
    assert!(err.to_string().contains("bbox escapes canvas"));
}

#[test]
fn vision_text_anchor_infers_center_only_inside_centered_region() {
    use glyph::vision::{VisionBox, VisionManifest, VisionObservation, manifest_to_glyph};
    let make = |kind: &str, text: Option<&str>, x, y, width, height| VisionObservation {
        kind: kind.into(),
        text: text.map(str::to_string),
        bbox: VisionBox {
            x,
            y,
            width,
            height,
        },
        confidence: 1.0,
        color: None,
        corner_radius: None,
        font_size: None,
        font_weight: None,
        baseline_y: None,
    };
    let manifest = VisionManifest {
        schema_version: 1,
        source: "/tmp/anchors.png".into(),
        width: 300,
        height: 200,
        background: Some("#ffffff".into()),
        observations: vec![
            make("rectangle", None, 20.0, 100.0, 200.0, 60.0),
            make("text", Some("Centered"), 80.0, 120.0, 80.0, 20.0),
            make("text", Some("Heading"), 30.0, 30.0, 90.0, 25.0),
        ],
    };
    let scene = parse_scene(&manifest_to_glyph(&manifest).unwrap(), "anchors.glyph").unwrap();
    let centered = scene
        .nodes
        .iter()
        .find(|n| n.authored.props.get("content").and_then(Value::text) == Some("Centered"))
        .unwrap();
    let heading = scene
        .nodes
        .iter()
        .find(|n| n.authored.props.get("content").and_then(Value::text) == Some("Heading"))
        .unwrap();
    assert_eq!(centered.authored.props["x"].number(), Some(120.0));
    assert_eq!(centered.authored.props["align"].text(), Some("center"));
    assert_eq!(heading.authored.props["x"].number(), Some(30.0));
    assert_eq!(heading.authored.props["align"].text(), Some("start"));
}

#[test]
fn vision_hybrid_import_preserves_raster_and_semantic_overlays() {
    use glyph::vision::{
        VisionBox, VisionImportMode, VisionManifest, VisionObservation, manifest_to_hybrid_glyph,
    };
    let manifest = VisionManifest {
        schema_version: 1,
        source: "/tmp/photo-reference.png".into(),
        width: 800,
        height: 600,
        background: None,
        observations: vec![VisionObservation {
            kind: "text".into(),
            text: Some("uncertain caption".into()),
            bbox: VisionBox {
                x: 60.0,
                y: 500.0,
                width: 180.0,
                height: 28.0,
            },
            confidence: 0.58,
            color: Some("#ffffff".into()),
            corner_radius: None,
            font_size: Some(24.0),
            font_weight: Some("500".into()),
            baseline_y: Some(522.0),
        }],
    };
    let source = manifest_to_hybrid_glyph(&manifest).unwrap();
    let scene = parse_scene(&source, "hybrid.glyph").unwrap();
    assert_eq!(
        scene.metadata["vision.reconstruction"],
        "hybrid-raster-semantic-overlays"
    );
    let raster = scene
        .nodes
        .iter()
        .find(|node| node.id == "vision_raster_base")
        .unwrap();
    assert_eq!(
        raster.authored.props["src"].text(),
        Some("/tmp/photo-reference.png")
    );
    assert_eq!(raster.authored.props["width"].number(), Some(800.0));
    assert_eq!(raster.authored.props["height"].number(), Some(600.0));
    let overlay = scene
        .nodes
        .iter()
        .find(|node| node.id != "vision_raster_base")
        .unwrap();
    assert_eq!(overlay.authored.props["opacity"].number(), Some(0.0));
    assert_eq!(overlay.authored.props["a11y_hidden"].text(), Some("true"));
    assert_eq!(VisionImportMode::Hybrid.to_string(), "hybrid");
}

#[test]
fn vision_auto_mode_distinguishes_structured_and_hybrid_inputs() {
    use glyph::vision::{
        VisionBox, VisionImportMode, VisionManifest, VisionObservation, choose_import_mode,
        source_for_manifest,
    };
    let make =
        |kind: &str, text: Option<&str>, x: f64, y: f64, confidence: f64| VisionObservation {
            kind: kind.into(),
            text: text.map(str::to_string),
            bbox: VisionBox {
                x,
                y,
                width: 100.0,
                height: 30.0,
            },
            confidence,
            color: None,
            corner_radius: Some(4.0),
            font_size: None,
            font_weight: None,
            baseline_y: None,
        };
    let structured = VisionManifest {
        schema_version: 1,
        source: "/tmp/ui.png".into(),
        width: 400,
        height: 300,
        background: Some("#ffffff".into()),
        observations: vec![
            make("rectangle", None, 20.0, 20.0, 0.96),
            make("text", Some("Title"), 40.0, 50.0, 0.98),
            make("text", Some("Body"), 40.0, 90.0, 0.95),
            make("rectangle", None, 20.0, 140.0, 0.93),
        ],
    };
    assert_eq!(
        choose_import_mode(&structured).unwrap(),
        VisionImportMode::Structured
    );
    let (structured_source, mode) =
        source_for_manifest(&structured, VisionImportMode::Auto).unwrap();
    assert_eq!(mode, VisionImportMode::Structured);
    assert!(!structured_source.contains("vision_raster_base"));

    let sparse = VisionManifest {
        schema_version: 1,
        source: "/tmp/photo.png".into(),
        width: 400,
        height: 300,
        background: None,
        observations: vec![make("text", Some("maybe"), 20.0, 240.0, 0.55)],
    };
    assert_eq!(
        choose_import_mode(&sparse).unwrap(),
        VisionImportMode::Hybrid
    );
    let (hybrid_source, mode) = source_for_manifest(&sparse, VisionImportMode::Auto).unwrap();
    assert_eq!(mode, VisionImportMode::Hybrid);
    assert!(hybrid_source.contains("vision_raster_base"));
}

#[test]
fn explicit_units_are_typed_preserved_and_canonicalized() {
    let src = r##"scene "Units" {
  canvas 400px 300px;
  rect box @id("box") { x 20px; y 30px; width 100px; height 80px; opacity 50%; }
  constrain box.left >= 10px @required;
  animate box.x from 20px to 220px duration 500ms easing "linear";
}"##;
    let mut scene = parse_scene(src, "units.glyph").unwrap();
    assert_eq!(scene.canvas.width_unit, Some(glyph::Unit::Px));
    assert_eq!(scene.nodes[0].authored.props["x"].number(), Some(20.0));
    assert_eq!(scene.nodes[0].authored.props["opacity"].number(), Some(0.5));
    assert_eq!(scene.animations[0].duration, 0.5);
    assert_eq!(scene.animations[0].duration_unit, Some(glyph::Unit::Ms));
    let report = solve_scene(&mut scene);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let sampled = glyph::sample_motion(&scene, 0.25).unwrap();
    assert!((sampled.nodes[0].derived.props["x"] - 120.0).abs() < 0.001);
}

#[test]
fn dimensionally_invalid_source_is_rejected() {
    let bad_property = r#"scene "Bad" { canvas 100px 100px; rect a { x 20deg; y 0px; width 10px; height 10px; } }"#;
    let err = parse_scene(bad_property, "bad-property.glyph").unwrap_err();
    assert!(err.to_string().contains("expects Length"), "{err}");

    let bad_constraint = r#"scene "Bad" { canvas 100px 100px; rect a { x 0px; y 0px; width 10px; height 10px; } constrain a.left == 30deg @required; }"#;
    let err = parse_scene(bad_constraint, "bad-constraint.glyph").unwrap_err();
    assert!(err.to_string().contains("dimension mismatch"), "{err}");
}

#[test]
fn patch_transactions_enforce_property_dimensions() {
    let mut scene = parse_scene(
        r#"scene "Patch Units" { canvas 100 100; rect a @id("a") { x 0; y 0; width 10; height 10; } }"#,
        "patch-units.glyph",
    )
    .unwrap();
    let err = apply_transaction(
        &mut scene,
        1,
        &[PatchOperation::Set {
            node: "a".into(),
            path: "props.x".into(),
            value: Value::Quantity(glyph::Quantity {
                value: 90.0,
                unit: glyph::Unit::Deg,
            }),
        }],
    )
    .unwrap_err();
    assert!(err.to_string().contains("expects Length"));
}

#[test]
fn source_round_trip_preserves_explicit_units() {
    let src = "scene \"Edit Units\" {\n  canvas 200px 200px;\n  rect a @id(\"a\") { x 10px; y 10px; width 20px; height 20px; }\n}\n";
    let edited = glyph::set_source_property(
        src,
        "a",
        "width",
        Value::Quantity(glyph::Quantity {
            value: 42.0,
            unit: glyph::Unit::Px,
        }),
    )
    .unwrap();
    assert!(edited.contains("width 42px;"));
    let reparsed = parse_scene(&edited, "edit-units.glyph").unwrap();
    assert_eq!(
        reparsed.nodes[0].authored.props["width"].number(),
        Some(42.0)
    );

    let text = glyph::parse_value_literal("steps").unwrap();
    assert_eq!(text.text(), Some("steps"));
}

#[test]
fn source_language_version_defaults_and_rejects_future_versions() {
    let implicit =
        parse_scene(r#"scene "Implicit" { canvas 100 100; }"#, "implicit.glyph").unwrap();
    assert_eq!(implicit.source_version, glyph::CURRENT_SOURCE_VERSION);

    let explicit = parse_scene(
        r#"scene "Explicit" @version(1) @id("explicit") { canvas 100 100; }"#,
        "explicit.glyph",
    )
    .unwrap();
    assert_eq!(explicit.source_version, 1);
    assert_eq!(explicit.id, "explicit");

    let err = parse_scene(
        r#"scene "Future" @version(2) { canvas 100 100; }"#,
        "future.glyph",
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("unsupported Glyph source version 2")
    );
}

#[test]
fn gsir_v1_migrates_deterministically_without_changing_revision() {
    let scene = parse_scene(
        r#"scene "Legacy" @version(1) { canvas 100 80; rect a @id("a") { x 1; y 2; width 3; height 4; } }"#,
        "legacy.glyph",
    )
    .unwrap();
    let mut legacy = serde_json::to_value(&scene).unwrap();
    legacy["schema_version"] = serde_json::json!(1);
    legacy.as_object_mut().unwrap().remove("source_version");
    let input = serde_json::to_string(&legacy).unwrap();

    let (migrated, report) = glyph::load_gsir_str(&input).unwrap();
    assert_eq!(report.from_schema, 1);
    assert_eq!(report.to_schema, glyph::CURRENT_GSIR_SCHEMA_VERSION);
    assert_eq!(report.steps, vec!["gsir-v1-to-v2"]);
    assert_eq!(migrated.revision, scene.revision);
    assert_eq!(migrated.source_version, 1);

    let current = serde_json::to_string(&migrated).unwrap();
    let (_, second_report) = glyph::load_gsir_str(&current).unwrap();
    assert!(!second_report.migrated);
    assert!(second_report.steps.is_empty());
}

#[test]
fn gsir_loader_rejects_missing_or_future_schema_versions() {
    let missing = r#"{"revision":1,"id":"x"}"#;
    let err = glyph::load_gsir_str(missing).unwrap_err();
    assert!(err.to_string().contains("missing integer schema_version"));

    let future = serde_json::json!({"schema_version": 999});
    let err = glyph::load_gsir_str(&future.to_string()).unwrap_err();
    assert!(
        err.to_string()
            .contains("unsupported GSIR schema version 999")
    );
}

#[test]
fn duplicate_source_version_attribute_is_rejected() {
    let err = parse_scene(
        r#"scene "Duplicate" @version(1) @version(1) { canvas 100 100; }"#,
        "duplicate-version.glyph",
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("duplicate scene @version attribute")
    );
}

#[test]
fn typed_component_interfaces_variants_slots_and_overrides_compose() {
    let src = r##"scene "Components" {
  canvas 600px 400px;
  component button {
    param width: Length -> bg.width = 120px;
    param fill: Color -> bg.fill = "#223344";
    slot label: Text -> label.content = "Button";
    variant compact { set bg.width 96px; set bg.height 36px; }
    rect bg @id("bg") { x 0px; y 0px; width 120px; height 44px; rx 4px; fill "#000000"; }
    text label @id("label") { content "Button"; x 10px; y 28px; size 20px; fill "#ffffff"; }
  }
  instance button primary @id("primary") {
    variant compact;
    param width 160px;
    param fill "#556677";
    slot label "Continue";
    override bg.rx 12px;
    x 40px; y 50px;
  }
}"##;
    let scene = parse_scene(src, "components.glyph").unwrap();
    assert_eq!(scene.components[0].parameters.len(), 2);
    assert_eq!(scene.components[0].slots.len(), 1);
    assert_eq!(scene.components[0].variants.len(), 1);
    let bg = scene
        .nodes
        .iter()
        .find(|node| node.id == "primary/bg")
        .unwrap();
    assert_eq!(bg.authored.props["width"].number(), Some(160.0));
    assert_eq!(bg.authored.props["height"].number(), Some(36.0));
    assert_eq!(bg.authored.props["rx"].number(), Some(12.0));
    assert_eq!(bg.authored.props["fill"].text(), Some("#556677"));
    assert_eq!(bg.authored.props["x"].number(), Some(40.0));
    assert_eq!(bg.authored.props["y"].number(), Some(50.0));
    assert!(bg.roles.iter().any(|role| role == "variant:compact"));

    let label = scene
        .nodes
        .iter()
        .find(|node| node.id == "primary/label")
        .unwrap();
    assert_eq!(label.authored.props["content"].text(), Some("Continue"));
    assert_eq!(label.authored.props["x"].number(), Some(50.0));
    assert_eq!(label.authored.props["y"].number(), Some(78.0));
}

#[test]
fn typed_component_required_bindings_and_types_fail_closed() {
    let missing = r#"scene "Missing" { canvas 100 100; component c { param width: Length -> box.width; rect box @id("box") { x 0; y 0; width 10; height 10; } } instance c i { } }"#;
    let err = parse_scene(missing, "missing-component-param.glyph").unwrap_err();
    assert!(
        err.to_string().contains("missing required parameter width"),
        "{err}"
    );

    let bad_assignment = r#"scene "Bad Type" { canvas 100 100; component c { param width: Length -> box.width = 10px; rect box @id("box") { x 0; y 0; width 10px; height 10px; } } instance c i { param width 30deg; } }"#;
    let err = parse_scene(bad_assignment, "bad-component-param.glyph").unwrap_err();
    assert!(
        err.to_string().contains("parameter width expects Length"),
        "{err}"
    );

    let bad_declaration = r#"scene "Bad Declaration" { canvas 100 100; component c { param turn: Angle -> box.width = 10deg; rect box @id("box") { x 0; y 0; width 10px; height 10px; } } }"#;
    let err = parse_scene(bad_declaration, "bad-component-declaration.glyph").unwrap_err();
    assert!(
        err.to_string().contains("declares Angle") && err.to_string().contains("Length"),
        "{err}"
    );
}

#[test]
fn typed_component_unknown_variants_and_local_targets_are_rejected() {
    let unknown_variant = r#"scene "Variant" { canvas 100 100; component c { rect box @id("box") { x 0; y 0; width 10; height 10; } } instance c i { variant missing; } }"#;
    let err = parse_scene(unknown_variant, "unknown-variant.glyph").unwrap_err();
    assert!(err.to_string().contains("unknown variant missing"), "{err}");

    let bad_target = r#"scene "Target" { canvas 100 100; component c { slot label: Text -> missing.content = "Hi"; text label @id("label") { content "Hi"; x 0; y 10; size 10; } } }"#;
    let err = parse_scene(bad_target, "bad-target.glyph").unwrap_err();
    assert!(
        err.to_string()
            .contains("unknown component-local node missing"),
        "{err}"
    );
}

#[test]
fn project_imports_namespace_components_and_styles() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("glyph.project.json"), r#"{"version":1}"#).unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(
        dir.path().join("lib/ui.glyph"),
        r##"scene "UI" { canvas 10 10; style accent { fill "#336699"; } component badge { rect bg @id("bg") { use accent; x 0; y 0; width 80; height 30; } } }"##,
    ).unwrap();
    fs::write(
        dir.path().join("main.glyph"),
        r#"import "lib/ui.glyph" as ui; scene "Main" { canvas 300 200; instance ui/badge b @id("b") { x 20; y 30; } }"#,
    ).unwrap();
    let scene = glyph::load_project_scene(&dir.path().join("main.glyph")).unwrap();
    assert_eq!(scene.imports.len(), 1);
    assert!(scene.components.iter().any(|c| c.name == "ui/badge"));
    assert!(scene.styles.contains_key("ui/accent"));
    let bg = scene.nodes.iter().find(|n| n.id == "b/bg").unwrap();
    assert_eq!(bg.authored.props["fill"].text(), Some("#336699"));
    assert_eq!(bg.authored.props["x"].number(), Some(20.0));
}

#[test]
fn project_imports_reject_escape_cycles_and_duplicate_aliases() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("glyph.project.json"), r#"{"version":1}"#).unwrap();
    fs::write(
        dir.path().join("a.glyph"),
        r#"import "b.glyph" as b; scene "A" { canvas 10 10; }"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("b.glyph"),
        r#"import "a.glyph" as a; scene "B" { canvas 10 10; }"#,
    )
    .unwrap();
    let err = glyph::load_project_scene(&dir.path().join("a.glyph")).unwrap_err();
    assert!(err.to_string().contains("import cycle detected"), "{err}");

    fs::write(
        dir.path().join("ok.glyph"),
        r#"scene "OK" { canvas 10 10; }"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("dup.glyph"),
        r#"import "ok.glyph" as x; import "ok.glyph" as x; scene "D" { canvas 10 10; }"#,
    )
    .unwrap();
    let err = glyph::load_project_scene(&dir.path().join("dup.glyph")).unwrap_err();
    assert!(
        err.to_string().contains("duplicate import alias x"),
        "{err}"
    );
}

#[test]
fn source_edit_preserves_imported_source_syntax() {
    let src = "import \"lib/ui.glyph\" as ui;\nscene \"Edit\" {\n  canvas 100 100;\n  rect box @id(\"box\") { x 1; y 2; width 10; height 10; }\n}\n";
    let edited = glyph::set_source_property(src, "box", "width", Value::Number(20.0)).unwrap();
    assert!(edited.starts_with("import \"lib/ui.glyph\" as ui;"));
    assert!(edited.contains("width 20;"));
}

#[test]
fn project_import_rejects_root_escape() {
    use std::fs;
    let outer = tempfile::tempdir().unwrap();
    let project = outer.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("glyph.project.json"), r#"{"version":1}"#).unwrap();
    fs::write(
        outer.path().join("outside.glyph"),
        r#"scene "Outside" { canvas 10 10; }"#,
    )
    .unwrap();
    fs::write(
        project.join("main.glyph"),
        r#"import "../outside.glyph" as outside; scene "Main" { canvas 10 10; }"#,
    )
    .unwrap();
    let err = glyph::load_project_scene(&project.join("main.glyph")).unwrap_err();
    assert!(
        err.to_string().contains("escapes Glyph project root"),
        "{err}"
    );
}

#[test]
fn project_manifest_validates_hashed_assets_and_builds_deterministic_bundle() {
    use sha2::{Digest, Sha256};
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::create_dir_all(dir.path().join("assets")).unwrap();
    fs::write(
        dir.path().join("lib/ui.glyph"),
        r##"scene "UI" { canvas 10 10; style accent { fill "#123456"; } }"##,
    )
    .unwrap();
    fs::write(
        dir.path().join("main.glyph"),
        r#"import "lib/ui.glyph" as ui; scene "Main" { canvas 100 80; rect box { x 0; y 0; width 10; height 10; } }"#,
    )
    .unwrap();
    let asset = b"glyph-asset-v1";
    fs::write(dir.path().join("assets/hero.bin"), asset).unwrap();
    let digest = format!("{:x}", Sha256::digest(asset));
    fs::write(
        dir.path().join("glyph.project.json"),
        format!(
            "{{\"version\":1,\"entry\":\"main.glyph\",\"assets\":{{\"hero\":{{\"path\":\"assets/hero.bin\",\"sha256\":\"{digest}\"}}}}}}"
        ),
    )
    .unwrap();

    let report = glyph::validate_project(&dir.path().join("main.glyph")).unwrap();
    assert_eq!(report.sources.len(), 2);
    assert_eq!(report.assets.len(), 1);
    assert_eq!(report.assets[0].name, "hero");
    assert_eq!(report.assets[0].sha256, digest);

    let a = glyph::project_bundle_manifest(&dir.path().join("main.glyph")).unwrap();
    let b = glyph::project_bundle_manifest(&dir.path().join("main.glyph")).unwrap();
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    assert_eq!(a.root, ".");
    assert_eq!(a.entry, "main.glyph");

    let bundle_a = dir.path().join("bundle-a");
    let bundle_b = dir.path().join("bundle-b");
    glyph::write_project_bundle(&dir.path().join("main.glyph"), &bundle_a).unwrap();
    glyph::write_project_bundle(&dir.path().join("main.glyph"), &bundle_b).unwrap();
    assert!(bundle_a.join("main.glyph").is_file());
    assert!(bundle_a.join("lib/ui.glyph").is_file());
    assert_eq!(fs::read(bundle_a.join("assets/hero.bin")).unwrap(), asset);
    assert_eq!(
        fs::read(bundle_a.join("glyph.bundle.json")).unwrap(),
        fs::read(bundle_b.join("glyph.bundle.json")).unwrap()
    );
    let bundled = glyph::validate_project(&bundle_a.join("main.glyph")).unwrap();
    assert_eq!(bundled.sources.len(), 2);
    assert_eq!(bundled.assets[0].sha256, digest);
    let err = glyph::write_project_bundle(&dir.path().join("main.glyph"), &bundle_a).unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
}

#[test]
fn project_assets_fail_closed_on_hash_mismatch_and_root_escape() {
    use std::fs;
    let outer = tempfile::tempdir().unwrap();
    let project = outer.path().join("project");
    fs::create_dir_all(project.join("assets")).unwrap();
    fs::write(
        project.join("main.glyph"),
        r#"scene "Main" { canvas 10 10; }"#,
    )
    .unwrap();
    fs::write(project.join("assets/a.bin"), b"actual").unwrap();
    fs::write(
        project.join("glyph.project.json"),
        r#"{"version":1,"entry":"main.glyph","assets":{"a":{"path":"assets/a.bin","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}}"#,
    )
    .unwrap();
    let err = glyph::validate_project(&project.join("main.glyph")).unwrap_err();
    assert!(err.to_string().contains("hash mismatch"), "{err}");

    fs::write(outer.path().join("outside.bin"), b"outside").unwrap();
    fs::write(
        project.join("glyph.project.json"),
        r#"{"version":1,"entry":"main.glyph","assets":{"outside":{"path":"../outside.bin","sha256":"d9298a10d1b0735837dc4bd85dac641b0f6e8e66b74f0f0dabeec0131db9a999"}}}"#,
    )
    .unwrap();
    let err = glyph::validate_project(&project.join("main.glyph")).unwrap_err();
    assert!(
        err.to_string().contains("escapes Glyph project root"),
        "{err}"
    );
}

#[test]
fn image_primitive_and_affine_transform_lower_deterministically() {
    let src = r#"scene "Media" { canvas 400 300; image hero @id("hero") { src "assets/hero.png"; fit "cover"; x 20; y 30; width 200; height 120; translate_x 5px; translate_y 6px; rotate 15deg; scale_x 1.25; scale_y 0.75; origin_x 100px; origin_y 60px; } }"#;
    let mut scene = parse_scene(src, "media.glyph").unwrap();
    solve_scene(&mut scene);
    let display = glyph::build_display_list(&scene);
    let item = &display.items[0];
    assert_eq!(item.transform.translate_x, 5.0);
    assert_eq!(item.transform.rotate_deg, 15.0);
    match &item.primitive {
        glyph::DisplayPrimitive::Image {
            src,
            fit,
            width,
            height,
            ..
        } => {
            assert_eq!(src, "assets/hero.png");
            assert_eq!(fit, "cover");
            assert_eq!((*width, *height), (200.0, 120.0));
        }
        other => panic!("expected image primitive, got {other:?}"),
    }
    let svg = render_svg(&scene);
    assert!(svg.contains("<image id=\"hero\""));
    assert!(svg.contains("preserveAspectRatio=\"xMidYMid slice\""));
    assert!(svg.contains("rotate(15) scale(1.25 0.75)"));
}

#[test]
fn clip_and_mask_targets_resolve_into_real_svg_defs() {
    let src = r##"scene "Defs" {
        canvas 300 200;
        rect clip_shape @id("clip_shape") { x 20; y 20; width 120; height 90; rx 12; }
        ellipse mask_shape @id("mask_shape") { cx 150; cy 100; rx 90; ry 70; fill "#ffffff"; }
        rect card @id("card") { x 0; y 0; width 280; height 180; fill "#336699"; clip clip_shape; mask mask_shape; }
    }"##;
    let mut scene = parse_scene(src, "defs.glyph").unwrap();
    solve_scene(&mut scene);
    let display = glyph::build_display_list(&scene);
    assert!(
        display
            .items
            .iter()
            .find(|i| i.id == "clip_shape")
            .unwrap()
            .definition_only
    );
    assert!(
        display
            .items
            .iter()
            .find(|i| i.id == "mask_shape")
            .unwrap()
            .definition_only
    );
    assert!(
        !display
            .items
            .iter()
            .find(|i| i.id == "card")
            .unwrap()
            .definition_only
    );
    let svg = render_svg(&scene);
    assert!(svg.contains("<clipPath id=\"clip-clip_shape\">"), "{svg}");
    assert!(svg.contains("<mask id=\"mask-mask_shape\">"), "{svg}");
    assert!(svg.contains("clip-path=\"url(#clip-clip_shape)\""), "{svg}");
    assert!(svg.contains("mask=\"url(#mask-mask_shape)\""), "{svg}");
    assert_eq!(svg.matches("id=\"card\"").count(), 1);
    assert!(!svg.contains("id=\"clip_shape\""));
    assert!(!svg.contains("id=\"mask_shape\""));
}

#[test]
fn clip_mask_and_paint_references_fail_closed() {
    let missing_clip =
        r#"scene "Bad" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; clip ghost; } }"#;
    let err = parse_scene(missing_clip, "missing-clip.glyph").unwrap_err();
    assert!(
        err.to_string().contains("missing clip target ghost"),
        "{err}"
    );

    let self_mask = r#"scene "Bad" { canvas 100 100; rect a @id("a") { x 0; y 0; width 10; height 10; mask a; } }"#;
    let err = parse_scene(self_mask, "self-mask.glyph").unwrap_err();
    assert!(err.to_string().contains("cannot mask itself"), "{err}");

    let missing_paint = r#"scene "Bad" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; fill_paint ghost; } }"#;
    let err = parse_scene(missing_paint, "missing-paint.glyph").unwrap_err();
    assert!(
        err.to_string().contains("missing fill_paint ghost"),
        "{err}"
    );
}

#[test]
fn typed_linear_and_radial_gradients_lower_to_svg_and_commander_draw() {
    let src = r##"scene "Paint" {
        canvas 400 240;
        paint sunset linear {
            from 0% 0%;
            to 100% 0%;
            stop 0% "#ff6633";
            stop 100% "#6633ff" opacity 80%;
        }
        paint glow radial {
            center 50% 50%;
            radius 50%;
            stop 0% "#ffffff";
            stop 100% "#000000" opacity 0;
        }
        rect band @id("band") { x 20; y 20; width 360; height 80; fill_paint sunset; }
        ellipse ring @id("ring") { cx 200; cy 170; rx 70; ry 45; fill "none"; stroke_paint glow; stroke_width 8; }
    }"##;
    let mut scene = parse_scene(src, "paint.glyph").unwrap();
    solve_scene(&mut scene);
    assert!(matches!(
        scene.paints["sunset"],
        glyph::Paint::Linear { .. }
    ));
    assert!(matches!(scene.paints["glow"], glyph::Paint::Radial { .. }));
    let display = glyph::build_display_list(&scene);
    assert_eq!(display.schema_version, 3);
    assert!(display.items[0].style.fill_paint.is_some());
    assert!(display.items[1].style.stroke_paint.is_some());
    let svg = render_svg(&scene);
    assert!(
        svg.contains("<linearGradient id=\"paint-fill-band\""),
        "{svg}"
    );
    assert!(
        svg.contains("<radialGradient id=\"paint-stroke-ring\""),
        "{svg}"
    );
    assert!(svg.contains("fill=\"url(#paint-fill-band)\""), "{svg}");
    assert!(svg.contains("stroke=\"url(#paint-stroke-ring)\""), "{svg}");
    let draw = glyph::commander_draw_document(&scene);
    assert_eq!(draw["nodes"][0]["style"]["fill"]["type"], "linear");
    assert_eq!(draw["nodes"][1]["style"]["stroke"]["type"], "radial");
}

#[test]
fn gradients_reject_invalid_stop_contracts() {
    let one_stop = r##"scene "Bad" { canvas 100 100; paint p linear { stop 0% "#000"; } rect a { x 0; y 0; width 10; height 10; fill_paint p; } }"##;
    let err = parse_scene(one_stop, "one-stop.glyph").unwrap_err();
    assert!(
        err.to_string().contains("requires at least two stops"),
        "{err}"
    );

    let unordered = r##"scene "Bad" { canvas 100 100; paint p linear { stop 80% "#000"; stop 20% "#fff"; } rect a { x 0; y 0; width 10; height 10; fill_paint p; } }"##;
    let err = parse_scene(unordered, "unordered.glyph").unwrap_err();
    assert!(err.to_string().contains("nondecreasing offset"), "{err}");
}

#[test]
fn commander_bridge_uses_real_image_href_and_affine_matrix() {
    let src = r#"scene "Media Bridge" { canvas 200 120; image hero @id("hero") { src "assets/hero.png"; fit "cover"; x 10; y 20; width 100; height 60; translate_x 5px; rotate 15deg; scale_x 1.25; scale_y 0.75; origin_x 50px; origin_y 30px; } }"#;
    let mut scene = parse_scene(src, "media-bridge.glyph").unwrap();
    solve_scene(&mut scene);
    let draw = glyph::commander_draw_document(&scene);
    assert_eq!(draw["nodes"][0]["href"], "assets/hero.png");
    assert_eq!(draw["nodes"][0]["preserveAspectRatio"], "xMidYMid slice");
    assert_eq!(
        draw["nodes"][0]["transform"]["matrix"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert!(draw["nodes"][0].get("src").is_none());
}

#[test]
fn project_imports_namespace_paints_used_by_components() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("glyph.project.json"), r#"{"version":1}"#).unwrap();
    fs::create_dir_all(dir.path().join("lib")).unwrap();
    fs::write(
        dir.path().join("lib/ui.glyph"),
        r##"scene "UI" {
            canvas 10 10;
            paint accent linear { stop 0% "#112233"; stop 100% "#99aabb"; }
            component badge { rect bg @id("bg") { x 0; y 0; width 80; height 30; fill_paint accent; } }
        }"##,
    )
    .unwrap();
    fs::write(
        dir.path().join("main.glyph"),
        r#"import "lib/ui.glyph" as ui; scene "Main" { canvas 300 200; instance ui/badge b @id("b") { x 20; y 30; } }"#,
    )
    .unwrap();
    let scene = glyph::load_project_scene(&dir.path().join("main.glyph")).unwrap();
    assert!(scene.paints.contains_key("ui/accent"));
    let bg = scene.nodes.iter().find(|n| n.id == "b/bg").unwrap();
    assert_eq!(bg.authored.props["fill_paint"].text(), Some("ui/accent"));
    let display = glyph::build_display_list(&scene);
    assert!(display.items[0].style.fill_paint.is_some());
}

#[test]
fn renderer_conformance_report_marks_backend_degradation_and_unsupported_features() {
    let src = r##"scene "Conformance Limits" {
        canvas 320 220;
        rect clip_shape @id("clip_shape") { x 20; y 20; width 120; height 120; translate_x 5px; }
        ellipse mask_shape @id("mask_shape") { cx 200; cy 90; rx 55; ry 55; fill "#ffffff"; }
        rect clipped @id("clipped") { x 20; y 20; width 120; height 120; fill "#ff0000"; clip "clip_shape"; }
        rect masked @id("masked") { x 150; y 30; width 100; height 100; fill "#0000ff"; mask "mask_shape"; }
    }"##;
    let mut scene = parse_scene(src, "conformance-limits.glyph").unwrap();
    let solve = solve_scene(&mut scene);
    assert!(solve.unresolved.is_empty(), "{solve:?}");
    let report = glyph::renderer_conformance_report(&scene);
    assert_eq!(report.display_list_schema, 3);
    assert_eq!(report.svg.overall, glyph::ConformanceStatus::Exact);
    assert_eq!(
        report.commander_draw.overall,
        glyph::ConformanceStatus::Unsupported
    );
    assert!(
        report
            .commander_draw
            .issues
            .iter()
            .any(|issue| issue.feature == "mask" && issue.item_id == "masked")
    );
    assert!(
        report
            .commander_draw
            .issues
            .iter()
            .any(|issue| issue.feature == "transformed_clip" && issue.item_id == "clipped")
    );
    assert!(!report.svg.pixel_fidelity_guaranteed);
}

#[test]
fn svg_and_commander_draw_preserve_supported_display_semantics_from_one_scene() {
    let src = r##"scene "Backend Contract" {
        canvas 640 360;
        paint band linear { from 0% 0%; to 100% 0%; stop 0% "#ff6633"; stop 100% "#6633ff"; }
        rect clip_shape @id("clip_shape") { x 30; y 30; width 260; height 160; rx 20; }
        rect card @id("card") { role "surface"; x 30; y 30; width 260; height 160; rx 20; fill_paint band; stroke "#111111"; stroke_width 3; opacity 0.8; clip "clip_shape"; translate_x 4px; translate_y 6px; rotate 5deg; origin_x 130px; origin_y 80px; }
        image hero @id("hero") { x 330; y 30; width 260; height 160; src "assets/hero.png"; fit "cover"; }
        text label @id("label") { content "office ffi"; x 40; y 250; size 32; font "Helvetica"; weight "700"; font_style "italic"; letter_spacing 2px; ligatures "none"; fill "#222222"; }
    }"##;
    let mut scene = parse_scene(src, "backend-contract.glyph").unwrap();
    let solve = solve_scene(&mut scene);
    assert!(solve.unresolved.is_empty(), "{solve:?}");
    let display = glyph::build_display_list(&scene);
    let report = glyph::renderer_conformance_report(&scene);
    assert_eq!(report.svg.overall, glyph::ConformanceStatus::Degraded);
    assert_eq!(
        report.commander_draw.overall,
        glyph::ConformanceStatus::Degraded
    );
    assert!(
        !report
            .commander_draw
            .issues
            .iter()
            .any(|issue| issue.status == glyph::ConformanceStatus::Unsupported)
    );

    let svg = render_svg(&scene);
    assert!(
        svg.contains("<linearGradient id=\"paint-fill-card\""),
        "{svg}"
    );
    assert!(
        svg.contains("id=\"card\" data-glyph-role=\"surface\""),
        "{svg}"
    );
    assert!(svg.contains("clip-path=\"url(#clip-clip_shape)\""), "{svg}");
    assert!(svg.contains("transform=\"translate(4 6) translate(130 80) rotate(5) scale(1 1) translate(-130 -80)\""), "{svg}");
    assert!(
        svg.contains("preserveAspectRatio=\"xMidYMid slice\""),
        "{svg}"
    );
    assert!(svg.contains("letter-spacing=\"2\""), "{svg}");
    let draw = glyph::commander_draw_document(&scene);
    let draw_nodes = draw["nodes"].as_array().unwrap();
    let card = draw_nodes.iter().find(|node| node["id"] == "card").unwrap();
    assert_eq!(card["style"]["fill"]["type"], "linear");
    assert_eq!(card["style"]["stroke"], "#111111");
    assert_eq!(card["style"]["strokeWidth"], 3.0);
    assert_eq!(card["style"]["opacity"], 0.8);
    assert!(card["transform"]["matrix"].is_array());
    assert!(card["clipPath"].as_str().unwrap().starts_with('M'));
    let hero = draw_nodes.iter().find(|node| node["id"] == "hero").unwrap();
    assert_eq!(hero["preserveAspectRatio"], "xMidYMid slice");
    let label = draw_nodes
        .iter()
        .find(|node| node["id"] == "label")
        .unwrap();
    assert_eq!(label["letterSpacing"], 2.0);
    assert_eq!(label["fontStyle"], "italic");
    assert_eq!(label["glyphTextMetrics"]["ligatures"], "none");
    assert_eq!(display.object_map["card"].roles, vec!["surface"]);
    assert_eq!(
        draw["meta"]["glyphDisplayListSchema"],
        display.schema_version
    );
}

#[test]
fn target_strategy_separates_visual_exports_from_semantic_codegen() {
    let src = r##"scene "Targets" {
        canvas 200 120;
        rect card @id("card") { x 10; y 10; width 180; height 100; fill "#336699"; }
    }"##;
    let mut scene = parse_scene(src, "targets.glyph").unwrap();
    solve_scene(&mut scene);
    let report = glyph::target_strategy_report(&scene);
    let target = |name: &str| report.targets.iter().find(|t| t.target == name).unwrap();

    assert_eq!(
        target("svg").availability,
        glyph::TargetAvailability::Implemented
    );
    assert_eq!(target("svg").class, glyph::TargetClass::VisualExport);
    assert_eq!(target("svg").status, glyph::ConformanceStatus::Exact);
    assert_eq!(
        target("pdf").availability,
        glyph::TargetAvailability::Implemented
    );
    assert_eq!(target("pdf").class, glyph::TargetClass::VisualExport);
    assert_eq!(
        target("swiftui").availability,
        glyph::TargetAvailability::StrategyOnly
    );
    assert_eq!(target("swiftui").class, glyph::TargetClass::SemanticCodegen);
    assert_eq!(
        target("swiftui").status,
        glyph::ConformanceStatus::Unsupported
    );
    assert_eq!(
        target("web_canvas").availability,
        glyph::TargetAvailability::StrategyOnly
    );
}
#[test]
fn pdf_export_fails_closed_before_backend_for_unsupported_masks() {
    let src = r##"scene "Masked PDF" {
        canvas 200 120;
        ellipse mask_shape @id("mask_shape") { cx 100; cy 60; rx 70; ry 45; fill "#ffffff"; }
        rect card @id("card") { x 10; y 10; width 180; height 100; fill "#336699"; mask mask_shape; }
    }"##;
    let mut scene = parse_scene(src, "masked-pdf.glyph").unwrap();
    solve_scene(&mut scene);
    let err = glyph::bridge::export_pdf_with_commander_draw(
        &scene,
        "/tmp/glyph-unsupported-mask.pdf",
        "/definitely/missing/commander-draw",
    )
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot faithfully render Glyph mask"),
        "{err}"
    );
}

#[test]
fn direct_manipulation_edits_only_unconstrained_authored_axes_and_preserves_px_units() {
    let src = r##"scene "Drag" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 30px; width 100px; height 80px; fill "#123456"; }
        rect follower @id("follower") { x 180px; y 30px; width 50px; height 50px; }
        constrain follower.left == box.right + 10px @strong;
    }"##;
    let mut scene = parse_scene(src, "drag.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", 15.0, -5.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.blocked.is_empty());
    assert_eq!(plan.edits.len(), 2);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("x 35px;"), "{edited}");
    assert!(edited.contains("y 25px;"), "{edited}");
    assert!(edited.contains("constrain follower.left == box.right + 10px @strong;"));
    let mut reparsed = parse_scene(&edited, "drag.glyph").unwrap();
    let report = solve_scene(&mut reparsed);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    assert_eq!(
        reparsed
            .nodes
            .iter()
            .find(|n| n.id == "box")
            .unwrap()
            .derived
            .props["x"],
        35.0
    );
}
#[test]
fn direct_manipulation_updates_single_equality_constraint_offset_without_flattening_relation() {
    let src = r##"scene "Drag Related" {
        canvas 400 300;
        rect box @id("box") { x 20; y 30; width 100; height 80; }
        constrain box.center_x == canvas.center_x @required;
    }"##;
    let mut scene = parse_scene(src, "drag-related.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", 25.0, 10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].property, "y");
    assert_eq!(plan.constraint_edits.len(), 1);
    assert_eq!(plan.constraint_edits[0].constraint_id, "constraint.1");
    assert_eq!(plan.constraint_edits[0].next_offset, 25.0);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("y 40;"), "{edited}");
    assert!(
        edited.contains("constrain box.center_x == canvas.center_x + 25 @required;"),
        "{edited}"
    );
    let mut reparsed = parse_scene(&edited, "drag-related.glyph").unwrap();
    let report = solve_scene(&mut reparsed);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let box_node = reparsed.nodes.iter().find(|n| n.id == "box").unwrap();
    assert_eq!(box_node.derived.props["x"], 175.0);
    assert_eq!(box_node.derived.props["y"], 40.0);
}

#[test]
fn direct_manipulation_preserves_equality_with_slack_inequality_guard() {
    let src = r##"scene "Drag Guarded" {
        canvas 400 300;
        rect box @id("box") { x 20; y 30; width 100; height 80; }
        constrain box.center_x == canvas.center_x @required;
        constrain box.left >= 0 @strong;
    }"##;
    let mut scene = parse_scene(src, "drag-guarded.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", 25.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.constraint_edits.len(), 1);
    assert_eq!(plan.constraint_edits[0].constraint_id, "constraint.1");
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(
        edited.contains("canvas.center_x + 25 @required;"),
        "{edited}"
    );
    assert!(
        edited.contains("constrain box.left >= 0 @strong;"),
        "{edited}"
    );
}

#[test]
fn direct_manipulation_updates_all_compatible_equalities_on_one_axis() {
    let src = r##"scene "Drag Multi Equality" {
        canvas 400 300;
        rect box @id("box") { x 20; y 30; width 100; height 80; }
        constrain box.left == canvas.left + 20 @required;
        constrain box.right == canvas.left + 120 @required;
    }"##;
    let mut scene = parse_scene(src, "drag-multi-equality.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", 25.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.constraint_edits.len(), 2, "{plan:?}");
    assert_eq!(plan.constraint_edits[0].next_offset, 45.0);
    assert_eq!(plan.constraint_edits[1].next_offset, 145.0);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(
        edited.contains("box.left == canvas.left + 45 @required;"),
        "{edited}"
    );
    assert!(
        edited.contains("box.right == canvas.left + 145 @required;"),
        "{edited}"
    );
}

#[test]
fn direct_manipulation_treats_inequalities_as_guards_when_requested_move_is_feasible() {
    let src = r##"scene "Drag Guard Only" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 30px; width 100px; height 80px; }
        constrain box.left >= 0 @required;
        constrain box.right <= canvas.right @required;
    }"##;
    let mut scene = parse_scene(src, "drag-guard-only.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", 25.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.constraint_edits.is_empty());
    assert_eq!(plan.edits.len(), 1);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("x 45px;"), "{edited}");
}

#[test]
fn direct_manipulation_blocks_when_inequality_guard_prevents_exact_requested_move() {
    let src = r##"scene "Drag Guard Block" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 30px; width 100px; height 80px; }
        constrain box.left >= 0 @required;
    }"##;
    let mut scene = parse_scene(src, "drag-guard-block.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "box", -30.0, 0.0).unwrap();
    assert!(!plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.blocked[0].constraint_ids, vec!["constraint.1"]);
    assert!(
        plan.blocked[0].reason.contains("requested delta"),
        "{plan:?}"
    );
}

#[test]
fn constraint_offset_source_edit_updates_and_removes_only_the_offset() {
    let src = r##"scene "Offset Edit" {
        canvas 400 300;
        rect box @id("box") { x 20; y 30; width 100; height 80; }
        constrain box.center_y == canvas.center_y - 70px @strong;
    }"##;
    let edited =
        glyph::source_edit::set_source_constraint_offset(src, 1, -58.0, Some(glyph::Unit::Px))
            .unwrap();
    assert!(
        edited.contains("canvas.center_y - 58px @strong;"),
        "{edited}"
    );
    let cleared =
        glyph::source_edit::set_source_constraint_offset(&edited, 1, 0.0, Some(glyph::Unit::Px))
            .unwrap();
    assert!(cleared.contains("canvas.center_y  @strong;"), "{cleared}");
    parse_scene(&cleared, "offset-edit.glyph").unwrap();
}

#[test]
fn direct_resize_edits_free_size_and_preserves_px_units() {
    let src = r##"scene "Resize" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 30px; width 100px; height 80px; }
    }"##;
    let mut scene = parse_scene(src, "resize.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "box", 25.0, -10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 2);
    assert!(plan.constraint_edits.is_empty());
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("width 125px;"), "{edited}");
    assert!(edited.contains("height 70px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "resize.glyph").unwrap();
    let report = solve_scene(&mut reparsed);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
}

#[test]
fn direct_resize_maps_visual_ellipse_delta_to_radius_edits() {
    let src = r##"scene "Resize Ellipse" {
        canvas 400 300;
        ellipse orb @id("orb") { cx 100px; cy 100px; rx 50px; ry 40px; }
        constrain orb.width >= 80px @required;
    }"##;
    let mut scene = parse_scene(src, "resize-ellipse.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "orb", 20.0, -10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 2);
    assert_eq!(plan.edits[0].property, "rx");
    assert_eq!(plan.edits[0].value.number(), Some(60.0));
    assert_eq!(plan.edits[1].property, "ry");
    assert_eq!(plan.edits[1].value.number(), Some(35.0));
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("rx 60px;"), "{edited}");
    assert!(edited.contains("ry 35px;"), "{edited}");
}

#[test]
fn direct_resize_updates_single_size_equality_offset_and_blocks_ambiguous_size_intent() {
    let src = r##"scene "Resize Relations" {
        canvas 400 300;
        rect base @id("base") { x 20; y 20; width 120; height 80; }
        rect related @id("related") { x 180; y 20; width 100; height 80; }
        rect ambiguous @id("ambiguous") { x 20; y 140; width 100; height 60; }
        constrain related.width == base.width + 10px @strong;
        constrain ambiguous.width == base.width @required;
        constrain ambiguous.width >= 80px @strong;
    }"##;
    let mut scene = parse_scene(src, "resize-relations.glyph").unwrap();
    solve_scene(&mut scene);
    let related = glyph::plan_direct_resize(&scene, "related", 15.0, 0.0).unwrap();
    assert!(related.allowed, "{related:?}");
    assert!(related.edits.is_empty());
    assert_eq!(related.constraint_edits[0].next_offset, 25.0);
    let edited = glyph::apply_direct_resize_source(src, &related).unwrap();
    assert!(
        edited.contains("related.width == base.width + 25px @strong;"),
        "{edited}"
    );
    let ambiguous = glyph::plan_direct_resize(&scene, "ambiguous", 15.0, 0.0).unwrap();
    assert!(!ambiguous.allowed, "{ambiguous:?}");
    assert!(ambiguous.edits.is_empty());
    assert!(ambiguous.constraint_edits.is_empty());
    assert_eq!(
        ambiguous.blocked[0].constraint_ids,
        vec!["constraint.2", "constraint.3"]
    );
    let too_small = glyph::plan_direct_resize(&scene, "base", -200.0, 0.0).unwrap_err();
    assert!(
        too_small.to_string().contains("non-positive"),
        "{too_small}"
    );
}

#[test]
fn direct_resize_preserves_equality_with_slack_inequality_guard() {
    let src = r##"scene "Resize Guarded" {
        canvas 400 300;
        rect box @id("box") { x 20; y 20; width 100px; height 80px; }
        constrain box.width == 100px @required;
        constrain box.width >= 80px @strong;
    }"##;
    let mut scene = parse_scene(src, "resize-guarded.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "box", 20.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.constraint_edits.len(), 1);
    assert_eq!(plan.constraint_edits[0].constraint_id, "constraint.1");
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(
        edited.contains("box.width == 100px + 20 @required;"),
        "{edited}"
    );
    assert!(edited.contains("box.width >= 80px @strong;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "resize-guarded.glyph").unwrap();
    let report = solve_scene(&mut reparsed);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let box_node = reparsed.nodes.iter().find(|n| n.id == "box").unwrap();
    assert_eq!(box_node.derived.props["width"], 120.0);
}

#[test]
fn direct_resize_updates_all_compatible_equalities_on_one_axis() {
    let src = r##"scene "Resize Multi Equality" {
        canvas 400 300;
        rect box @id("box") { x 20; y 20; width 100px; height 80px; }
        constrain box.width == 100px @required;
        constrain box.width == canvas.width - 300px @required;
    }"##;
    let mut scene = parse_scene(src, "resize-multi-equality.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "box", 20.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.constraint_edits.len(), 2, "{plan:?}");
    assert_eq!(plan.constraint_edits[0].next_offset, 20.0);
    assert_eq!(plan.constraint_edits[1].next_offset, -280.0);
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(
        edited.contains("box.width == 100px + 20 @required;"),
        "{edited}"
    );
    assert!(
        edited.contains("box.width == canvas.width - 280px @required;"),
        "{edited}"
    );
}

#[test]
fn direct_resize_treats_inequalities_as_guards_when_requested_size_is_feasible() {
    let src = r##"scene "Resize Guard Only" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 20px; width 100px; height 80px; }
        constrain box.width >= 80px @required;
        constrain box.width <= 200px @required;
    }"##;
    let mut scene = parse_scene(src, "resize-guard-only.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "box", 20.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.constraint_edits.is_empty());
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].property, "width");
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("width 120px;"), "{edited}");
}

#[test]
fn direct_resize_blocks_when_inequality_guard_prevents_exact_requested_size() {
    let src = r##"scene "Resize Guard Block" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 20px; width 100px; height 80px; }
        constrain box.width >= 80px @required;
    }"##;
    let mut scene = parse_scene(src, "resize-guard-block.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "box", -30.0, 0.0).unwrap();
    assert!(!plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.blocked[0].constraint_ids, vec!["constraint.1"]);
    assert!(
        plan.blocked[0].reason.contains("requested delta"),
        "{plan:?}"
    );
}

#[test]
fn component_instance_drag_updates_unambiguous_scaled_parameters() {
    let src = r##"scene "Component Drag" {
        canvas 600 400;
        component card {
            param px: Length -> bg.x = 10px;
            param py: Length -> bg.y = 20px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
        }
        instance card card1 @id("card1") {
            x 100px; y 50px; scale 2;
            param px 15px; param py 25px;
        }
    }"##;
    let mut scene = parse_scene(src, "component-drag.glyph").unwrap();
    solve_scene(&mut scene);
    let before = scene.nodes.iter().find(|n| n.id == "card1/bg").unwrap();
    assert_eq!(before.derived.props["x"], 130.0);
    assert_eq!(before.derived.props["y"], 100.0);
    let plan = glyph::plan_direct_manipulation(&scene, "card1/bg", 20.0, -10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.instance_parameter_edits.len(), 2);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("param px 25px;"), "{edited}");
    assert!(edited.contains("param py 20px;"), "{edited}");
    assert!(edited.contains("param px: Length -> bg.x = 10px;"));
    let mut reparsed = parse_scene(&edited, "component-drag.glyph").unwrap();
    let report = solve_scene(&mut reparsed);
    assert!(report.required_conflicts.is_empty(), "{report:?}");
    let after = reparsed.nodes.iter().find(|n| n.id == "card1/bg").unwrap();
    assert_eq!(after.derived.props["x"], 150.0);
    assert_eq!(after.derived.props["y"], 90.0);
}

#[test]
fn component_instance_drag_inserts_default_parameter_override() {
    let src = r##"scene "Component Default Drag" {
        canvas 400 300;
        component card {
            param px: Length -> bg.x = 10px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
        }
        instance card card1 @id("card1") { x 100px; scale 2; }
    }"##;
    let mut scene = parse_scene(src, "component-default-drag.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "card1/bg", 20.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.instance_parameter_edits.len(), 1);
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("param px 20px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "component-default-drag.glyph").unwrap();
    solve_scene(&mut reparsed);
    let child = reparsed.nodes.iter().find(|n| n.id == "card1/bg").unwrap();
    assert_eq!(child.derived.props["x"], 140.0);
}

#[test]
fn component_instance_resize_updates_scaled_size_parameters() {
    let src = r##"scene "Component Resize" {
        canvas 500 300;
        component card {
            param w: Length -> bg.width = 100px;
            param h: Length -> bg.height = 60px;
            rect bg @id("bg") { x 0px; y 0px; width 100px; height 60px; }
        }
        instance card card1 @id("card1") {
            x 40px; y 30px; scale 2;
            param w 120px; param h 70px;
        }
    }"##;
    let mut scene = parse_scene(src, "component-resize.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "card1/bg", 20.0, -10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.instance_parameter_edits.len(), 2);
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("param w 130px;"), "{edited}");
    assert!(edited.contains("param h 65px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "component-resize.glyph").unwrap();
    solve_scene(&mut reparsed);
    let child = reparsed.nodes.iter().find(|n| n.id == "card1/bg").unwrap();
    assert_eq!(child.derived.props["width"], 260.0);
    assert_eq!(child.derived.props["height"], 130.0);
    let err = glyph::plan_direct_resize(&scene, "card1/bg", -260.0, 0.0).unwrap_err();
    assert!(err.to_string().contains("non-positive"), "{err}");
}

#[test]
fn component_inverse_manipulation_blocks_instance_override() {
    let src = r##"scene "Component Override" {
        canvas 400 300;
        component card {
            param px: Length -> bg.x = 10px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
        }
        instance card card1 @id("card1") { param px 15px; override bg.x 40px; }
    }"##;
    let mut scene = parse_scene(src, "component-override.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "card1/bg", 10.0, 0.0).unwrap();
    assert!(!plan.allowed, "{plan:?}");
    assert!(plan.instance_parameter_edits.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert!(plan.blocked[0].reason.contains("instance override"));
    let err = glyph::apply_direct_manipulation_source(src, &plan).unwrap_err();
    assert!(err.to_string().contains("blocked"), "{err}");
}

#[test]
fn component_inverse_manipulation_blocks_ambiguous_parameter_binding() {
    let src = r##"scene "Component Ambiguous" {
        canvas 400 300;
        component card {
            param first: Length -> bg.x = 10px;
            param second: Length -> bg.x = 10px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; }
        }
        instance card card1 @id("card1") { param first 15px; param second 15px; }
    }"##;
    let mut scene = parse_scene(src, "component-ambiguous.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "card1/bg", 10.0, 0.0).unwrap();
    assert!(!plan.allowed, "{plan:?}");
    assert!(plan.instance_parameter_edits.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert!(
        plan.blocked[0]
            .reason
            .contains("multiple component parameters")
    );
    assert!(plan.blocked[0].reason.contains("first"));
    assert!(plan.blocked[0].reason.contains("second"));
}

#[test]
fn framed_text_resize_updates_wrap_width_without_scaling_type() {
    let src = r##"scene "Text Resize" {
        canvas 500 300;
        text copy @id("copy") {
            role "copy";
            content "Semantic text frames should reflow without changing the type scale";
            x 40px; y 90px; size 24px; max_width 180px; line_height 30px; fill "#111111";
        }
    }"##;
    let mut scene = parse_scene(src, "text-resize.glyph").unwrap();
    solve_scene(&mut scene);
    let before = scene.nodes.iter().find(|node| node.id == "copy").unwrap();
    assert_eq!(glyph::query::bounds(before).unwrap()[2], 180.0);
    let before_size = before.authored.props["size"].number();
    let before_content = before.authored.props["content"].text().map(str::to_string);

    let plan = glyph::plan_direct_resize(&scene, "copy", 60.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 1);
    assert_eq!(plan.edits[0].property, "max_width");
    assert_eq!(plan.edits[0].value.number(), Some(240.0));

    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("max_width 240px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "text-resize.glyph").unwrap();
    solve_scene(&mut reparsed);
    let after = reparsed
        .nodes
        .iter()
        .find(|node| node.id == "copy")
        .unwrap();
    assert_eq!(after.authored.props["size"].number(), before_size);
    assert_eq!(
        after.authored.props["content"].text().map(str::to_string),
        before_content
    );
    assert_eq!(after.authored.props["max_width"].number(), Some(240.0));
    assert_eq!(glyph::query::bounds(after).unwrap()[2], 240.0);
}

#[test]
fn text_resize_fails_closed_when_frame_semantics_are_ambiguous() {
    let point_src = r##"scene "Point Text" {
        canvas 300 160;
        text label @id("label") { content "Point text"; x 20px; y 60px; size 24px; fill "#111"; }
    }"##;
    let mut point_scene = parse_scene(point_src, "point-text.glyph").unwrap();
    solve_scene(&mut point_scene);
    let point_plan = glyph::plan_direct_resize(&point_scene, "label", 30.0, 0.0).unwrap();
    assert!(!point_plan.allowed, "{point_plan:?}");
    assert!(point_plan.blocked[0].reason.contains("max_width"));

    let framed_src = r##"scene "Framed Text" {
        canvas 300 160;
        text label @id("label") { content "Framed text that can wrap"; x 20px; y 60px; size 24px; max_width 140px; fill "#111"; }
    }"##;
    let mut framed_scene = parse_scene(framed_src, "framed-text.glyph").unwrap();
    solve_scene(&mut framed_scene);
    let vertical = glyph::plan_direct_resize(&framed_scene, "label", 0.0, 20.0).unwrap();
    assert!(!vertical.allowed, "{vertical:?}");
    assert!(vertical.blocked[0].reason.contains("vertical text resize"));

    let transformed_src = r##"scene "Scaled Text" {
        canvas 300 160;
        text label @id("label") { content "Scaled text"; x 20px; y 60px; size 24px; max_width 140px; scale_x 1.5; fill "#111"; }
    }"##;
    let mut transformed_scene = parse_scene(transformed_src, "scaled-text.glyph").unwrap();
    solve_scene(&mut transformed_scene);
    let transformed = glyph::plan_direct_resize(&transformed_scene, "label", 20.0, 0.0).unwrap();
    assert!(!transformed.allowed, "{transformed:?}");
    assert!(transformed.blocked[0].reason.contains("affine transform"));
}

#[test]
fn component_text_frame_resize_inverts_unambiguous_max_width_parameter() {
    let src = r##"scene "Component Text Resize" {
        canvas 600 320;
        component card {
            param wrap: Length -> copy.max_width = 120px;
            text copy @id("copy") { content "Component text should preserve its parameterized frame"; x 0px; y 40px; size 20px; max_width 120px; fill "#111"; }
        }
        instance card card1 @id("card1") {
            x 60px; y 30px; scale 2;
            param wrap 130px;
        }
    }"##;
    let mut scene = parse_scene(src, "component-text-resize.glyph").unwrap();
    solve_scene(&mut scene);
    let child = scene
        .nodes
        .iter()
        .find(|node| node.id == "card1/copy")
        .unwrap();
    assert_eq!(child.authored.props["max_width"].number(), Some(260.0));

    let plan = glyph::plan_direct_resize(&scene, "card1/copy", 40.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.instance_parameter_edits.len(), 1);
    assert_eq!(plan.instance_parameter_edits[0].parameter, "wrap");
    assert_eq!(
        plan.instance_parameter_edits[0].next_value.number(),
        Some(150.0)
    );

    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("param wrap 150px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "component-text-resize.glyph").unwrap();
    solve_scene(&mut reparsed);
    let after = reparsed
        .nodes
        .iter()
        .find(|node| node.id == "card1/copy")
        .unwrap();
    assert_eq!(after.authored.props["max_width"].number(), Some(300.0));
    assert_eq!(glyph::query::bounds(after).unwrap()[2], 300.0);
}

#[test]
fn multi_edge_rect_resize_preserves_opposite_edges() {
    let src = r##"scene "Handle Rect" {
        canvas 400 300;
        rect box @id("box") { x 20px; y 30px; width 100px; height 80px; fill "#334455"; }
    }"##;
    let mut scene = parse_scene(src, "handle-rect.glyph").unwrap();
    solve_scene(&mut scene);

    let left =
        glyph::plan_direct_resize_from_handle(&scene, "box", glyph::ResizeHandle::Left, 15.0, 0.0)
            .unwrap();
    assert!(left.allowed, "{left:?}");
    assert_eq!(left.target_bounds, [35.0, 30.0, 85.0, 80.0]);
    let edited = glyph::apply_direct_handle_resize_source(src, &left).unwrap();
    assert!(edited.contains("x 35px;"), "{edited}");
    assert!(edited.contains("width 85px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "handle-rect.glyph").unwrap();
    solve_scene(&mut reparsed);
    assert_eq!(
        glyph::query::bounds(reparsed.nodes.iter().find(|node| node.id == "box").unwrap()),
        Some([35.0, 30.0, 85.0, 80.0])
    );

    let top_left = glyph::plan_direct_resize_from_handle(
        &scene,
        "box",
        glyph::ResizeHandle::TopLeft,
        10.0,
        20.0,
    )
    .unwrap();
    assert!(top_left.allowed, "{top_left:?}");
    assert_eq!(top_left.target_bounds, [30.0, 50.0, 90.0, 60.0]);
}

#[test]
fn multi_edge_ellipse_resize_moves_center_to_hold_opposite_edge() {
    let src = r##"scene "Handle Ellipse" {
        canvas 400 300;
        ellipse dot @id("dot") { cx 120px; cy 100px; rx 50px; ry 30px; fill "#445566"; }
    }"##;
    let mut scene = parse_scene(src, "handle-ellipse.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize_from_handle(
        &scene,
        "dot",
        glyph::ResizeHandle::BottomRight,
        20.0,
        10.0,
    )
    .unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.target_bounds, [70.0, 70.0, 120.0, 70.0]);
    let edited = glyph::apply_direct_handle_resize_source(src, &plan).unwrap();
    let mut reparsed = parse_scene(&edited, "handle-ellipse.glyph").unwrap();
    solve_scene(&mut reparsed);
    let dot = reparsed.nodes.iter().find(|node| node.id == "dot").unwrap();
    assert_eq!(glyph::query::bounds(dot), Some([70.0, 70.0, 120.0, 70.0]));
    assert_eq!(dot.authored.props["cx"].number(), Some(130.0));
    assert_eq!(dot.authored.props["cy"].number(), Some(105.0));
}

#[test]
fn multi_edge_path_resize_preserves_d_and_exact_anchor() {
    let src = r##"scene "Handle Path" {
        canvas 300 240;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; }
    }"##;
    let mut scene = parse_scene(src, "handle-path.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize_from_handle(
        &scene,
        "mark",
        glyph::ResizeHandle::TopLeft,
        10.0,
        5.0,
    )
    .unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.target_bounds, [20.0, 25.0, 40.0, 45.0]);
    let edited = glyph::apply_direct_handle_resize_source(src, &plan).unwrap();
    assert!(edited.contains("d \"M10 20 L60 20 L60 70 Z\";"), "{edited}");
    let mut reparsed = parse_scene(&edited, "handle-path.glyph").unwrap();
    solve_scene(&mut reparsed);
    assert_eq!(
        glyph::query::bounds(
            reparsed
                .nodes
                .iter()
                .find(|node| node.id == "mark")
                .unwrap()
        ),
        Some([20.0, 25.0, 40.0, 45.0])
    );
}

#[test]
fn multi_edge_text_frame_resize_preserves_right_edge() {
    let src = r##"scene "Handle Text" {
        canvas 400 220;
        text copy @id("copy") { content "Text frame handle semantics"; x 40px; y 80px; size 22px; max_width 180px; fill "#111"; }
    }"##;
    let mut scene = parse_scene(src, "handle-text.glyph").unwrap();
    solve_scene(&mut scene);
    let before =
        glyph::query::bounds(scene.nodes.iter().find(|node| node.id == "copy").unwrap()).unwrap();
    let plan =
        glyph::plan_direct_resize_from_handle(&scene, "copy", glyph::ResizeHandle::Left, 20.0, 0.0)
            .unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.target_bounds[0], before[0] + 20.0);
    assert_eq!(plan.target_bounds[2], 160.0);
    let edited = glyph::apply_direct_handle_resize_source(src, &plan).unwrap();
    assert!(edited.contains("max_width 160px;"), "{edited}");
    assert!(edited.contains("x 60px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "handle-text.glyph").unwrap();
    solve_scene(&mut reparsed);
    let after = glyph::query::bounds(
        reparsed
            .nodes
            .iter()
            .find(|node| node.id == "copy")
            .unwrap(),
    )
    .unwrap();
    assert!((after[0] - (before[0] + 20.0)).abs() < 0.0001);
    assert!((after[0] + after[2] - (before[0] + before[2])).abs() < 0.0001);
}

#[test]
fn multi_edge_resize_composes_size_and_position_constraints() {
    let src = r##"scene "Handle Constraints" {
        canvas 400 240;
        rect box @id("box") { x 150px; y 40px; width 100px; height 60px; fill "#111"; }
        constrain box.center_x == canvas.center_x @required;
    }"##;
    let mut scene = parse_scene(src, "handle-constraints.glyph").unwrap();
    solve_scene(&mut scene);
    let plan =
        glyph::plan_direct_resize_from_handle(&scene, "box", glyph::ResizeHandle::Left, 20.0, 0.0)
            .unwrap();
    assert!(plan.allowed, "{plan:?}");
    let movement = plan.movement_plan.as_ref().expect("anchor movement plan");
    assert_eq!(movement.constraint_edits.len(), 1);
    let edited = glyph::apply_direct_handle_resize_source(src, &plan).unwrap();
    assert!(
        edited.contains("constrain box.center_x == canvas.center_x + 10 @required;"),
        "{edited}"
    );
    let mut reparsed = parse_scene(&edited, "handle-constraints.glyph").unwrap();
    solve_scene(&mut reparsed);
    assert_eq!(
        glyph::query::bounds(reparsed.nodes.iter().find(|node| node.id == "box").unwrap()),
        Some([170.0, 40.0, 80.0, 60.0])
    );
}

#[test]
fn multi_edge_component_resize_inverts_position_and_size_parameters() {
    let src = r##"scene "Handle Component" {
        canvas 500 300;
        component card {
            param px: Length -> bg.x = 10px;
            param w: Length -> bg.width = 100px;
            rect bg @id("bg") { x 10px; y 20px; width 100px; height 60px; fill "#111"; }
        }
        instance card card1 @id("card1") { x 100px; scale 2; param px 10px; param w 100px; }
    }"##;
    let mut scene = parse_scene(src, "handle-component.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize_from_handle(
        &scene,
        "card1/bg",
        glyph::ResizeHandle::Left,
        20.0,
        0.0,
    )
    .unwrap();
    assert!(plan.allowed, "{plan:?}");
    let edited = glyph::apply_direct_handle_resize_source(src, &plan).unwrap();
    assert!(edited.contains("param px 20px;"), "{edited}");
    assert!(edited.contains("param w 90px;"), "{edited}");
    let mut reparsed = parse_scene(&edited, "handle-component.glyph").unwrap();
    solve_scene(&mut reparsed);
    assert_eq!(
        glyph::query::bounds(
            reparsed
                .nodes
                .iter()
                .find(|node| node.id == "card1/bg")
                .unwrap()
        ),
        Some([140.0, 40.0, 180.0, 120.0])
    );
}

#[test]
fn multi_edge_resize_rejects_non_positive_target_bounds() {
    let src = r##"scene "Handle Reject" {
        canvas 200 120;
        rect box @id("box") { x 20px; y 20px; width 50px; height 40px; }
    }"##;
    let mut scene = parse_scene(src, "handle-reject.glyph").unwrap();
    solve_scene(&mut scene);
    let err =
        glyph::plan_direct_resize_from_handle(&scene, "box", glyph::ResizeHandle::Left, 60.0, 0.0)
            .unwrap_err();
    assert!(err.to_string().contains("non-positive"), "{err}");
}

#[test]
fn object_bounds_include_images_paths_and_affine_translation() {
    let src = r##"scene "Bounds" {
        canvas 400 300;
        image hero @id("hero") { x 20px; y 30px; width 80px; height 50px; src "hero.png"; translate_x 7px; translate_y -3px; }
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; translate_x 15px; translate_y -5px; }
    }"##;
    let mut scene = parse_scene(src, "bounds.glyph").unwrap();
    solve_scene(&mut scene);
    let hero = scene.nodes.iter().find(|n| n.id == "hero").unwrap();
    let mark = scene.nodes.iter().find(|n| n.id == "mark").unwrap();
    assert_eq!(glyph::query::bounds(hero), Some([27.0, 27.0, 80.0, 50.0]));
    assert_eq!(glyph::query::bounds(mark), Some([25.0, 15.0, 50.0, 50.0]));
    let list = glyph::build_display_list(&scene);
    assert_eq!(
        list.object_map["hero"].bounds,
        Some([27.0, 27.0, 80.0, 50.0])
    );
    assert_eq!(
        list.object_map["mark"].bounds,
        Some([25.0, 15.0, 50.0, 50.0])
    );
}

#[test]
fn path_bounds_use_bezier_geometry_not_control_point_extents() {
    let src = r##"scene "Curve Bounds" {
        canvas 200 200;
        path curve @id("curve") { d "M0 0 C0 100 100 100 100 0"; stroke "#111111"; }
    }"##;
    let mut scene = parse_scene(src, "curve-bounds.glyph").unwrap();
    solve_scene(&mut scene);
    let curve = scene.nodes.iter().find(|n| n.id == "curve").unwrap();
    let bounds = glyph::query::bounds(curve).unwrap();
    assert!((bounds[0] - 0.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[1] - 0.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[2] - 100.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[3] - 75.0).abs() < 1e-9, "{bounds:?}");
}

#[test]
fn path_direct_manipulation_uses_affine_translation_without_rewriting_d() {
    let src = r##"scene "Path Drag" {
        canvas 300 200;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; }
    }"##;
    let mut scene = parse_scene(src, "path-drag.glyph").unwrap();
    solve_scene(&mut scene);
    assert_eq!(
        glyph::query::bounds(scene.nodes.iter().find(|n| n.id == "mark").unwrap()),
        Some([10.0, 20.0, 50.0, 50.0])
    );
    let plan = glyph::plan_direct_manipulation(&scene, "mark", 15.0, -5.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 2);
    assert_eq!(plan.edits[0].property, "translate_x");
    assert_eq!(plan.edits[1].property, "translate_y");
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("translate_x 15;"), "{edited}");
    assert!(edited.contains("translate_y -5;"), "{edited}");
    assert!(edited.contains("d \"M10 20 L60 20 L60 70 Z\";"), "{edited}");
    let mut reparsed = parse_scene(&edited, "path-drag.glyph").unwrap();
    solve_scene(&mut reparsed);
    let mark = reparsed.nodes.iter().find(|n| n.id == "mark").unwrap();
    assert_eq!(glyph::query::bounds(mark), Some([25.0, 15.0, 50.0, 50.0]));
    let svg = glyph::render_svg(&reparsed);
    assert!(svg.contains("translate(15 -5)"), "{svg}");
}

#[test]
fn path_direct_resize_uses_affine_scale_without_rewriting_d() {
    let src = r##"scene "Path Resize" {
        canvas 300 240;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; fill "#112233"; }
    }"##;
    let mut scene = parse_scene(src, "path-resize.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_resize(&scene, "mark", 25.0, 10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.edits.len(), 4);
    assert_eq!(plan.edits[0].property, "scale_x");
    assert_eq!(plan.edits[0].value.number(), Some(1.5));
    assert_eq!(plan.edits[1].property, "translate_x");
    assert_eq!(plan.edits[1].value.number(), Some(-5.0));
    assert_eq!(plan.edits[2].property, "scale_y");
    assert_eq!(plan.edits[2].value.number(), Some(1.2));
    assert_eq!(plan.edits[3].property, "translate_y");
    assert_eq!(plan.edits[3].value.number(), Some(-4.0));
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("d \"M10 20 L60 20 L60 70 Z\";"), "{edited}");
    let mut reparsed = parse_scene(&edited, "path-resize.glyph").unwrap();
    solve_scene(&mut reparsed);
    let mark = reparsed.nodes.iter().find(|n| n.id == "mark").unwrap();
    let bounds = glyph::query::bounds(mark).unwrap();
    assert!((bounds[0] - 10.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[1] - 20.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[2] - 75.0).abs() < 1e-9, "{bounds:?}");
    assert!((bounds[3] - 60.0).abs() < 1e-9, "{bounds:?}");
}

#[test]
fn path_direct_resize_supports_translate_relations_and_blocks_ambiguous_constraints() {
    let rotated = r##"scene "Rotated Path Resize" {
        canvas 300 240;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; rotate 15deg; }
    }"##;
    let mut rotated_scene = parse_scene(rotated, "path-resize-rotated.glyph").unwrap();
    solve_scene(&mut rotated_scene);
    let rotated_plan = glyph::plan_direct_resize(&rotated_scene, "mark", 10.0, 0.0).unwrap();
    assert!(!rotated_plan.allowed, "{rotated_plan:?}");
    assert!(rotated_plan.note.contains("rotated"), "{rotated_plan:?}");

    let constrained = r##"scene "Constrained Path Resize" {
        canvas 300 240;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; translate_x 5px; }
        constrain mark.translate_x == 5px @required;
    }"##;
    let mut constrained_scene = parse_scene(constrained, "path-resize-constrained.glyph").unwrap();
    solve_scene(&mut constrained_scene);
    let constrained_plan =
        glyph::plan_direct_resize(&constrained_scene, "mark", 10.0, 0.0).unwrap();
    assert!(constrained_plan.allowed, "{constrained_plan:?}");
    assert_eq!(constrained_plan.constraint_edits.len(), 1);
    assert_eq!(
        constrained_plan.constraint_edits[0].constraint_id,
        "constraint.1"
    );
    let edited = glyph::apply_direct_resize_source(constrained, &constrained_plan).unwrap();
    assert!(edited.contains("scale_x 1.2;"), "{edited}");
    assert!(
        edited.contains("constrain mark.translate_x == 5px - 2 @required;"),
        "{edited}"
    );
    assert!(edited.contains("d \"M10 20 L60 20 L60 70 Z\";"), "{edited}");

    let scale_constrained = r##"scene "Scale Constrained Path Resize" {
        canvas 300 240;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; scale_x 1; }
        constrain mark.scale_x == 1 @required;
    }"##;
    let mut scale_scene =
        parse_scene(scale_constrained, "path-resize-scale-constrained.glyph").unwrap();
    solve_scene(&mut scale_scene);
    let blocked = glyph::plan_direct_resize(&scale_scene, "mark", 10.0, 0.0).unwrap();
    assert!(!blocked.allowed, "{blocked:?}");
    assert_eq!(blocked.blocked[0].constraint_ids, vec!["constraint.1"]);
}

#[test]
fn path_drag_preserves_single_translate_equality_by_editing_offset() {
    let src = r##"scene "Constrained Path" {
        canvas 300 200;
        path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; translate_x 5px; }
        constrain mark.translate_x == 5px @required;
    }"##;
    let mut scene = parse_scene(src, "path-constrained.glyph").unwrap();
    solve_scene(&mut scene);
    let plan = glyph::plan_direct_manipulation(&scene, "mark", 12.0, 0.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty());
    assert_eq!(plan.constraint_edits.len(), 1);
    assert_eq!(plan.constraint_edits[0].constraint_id, "constraint.1");
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(
        edited.contains("constrain mark.translate_x == 5px + 12 @required;"),
        "{edited}"
    );
    assert!(edited.contains("translate_x 5px;"), "{edited}");
}

#[test]
fn constraint_source_edit_changes_only_selected_relation_strength_and_offset() {
    let src = "scene \"Constraint Edit\" {\n  canvas 320 220;\n  rect box @id(\"box\") { x 20px; y 30px; width 80px; height 60px; }\n  constrain box.center_x == canvas.center_x @required;\n  constrain box.left >= 10px @strong;\n}\n";
    let edited = glyph::set_source_constraint_relation(src, 1, glyph::Relation::Ge).unwrap();
    let edited = glyph::set_source_constraint_strength(&edited, 1, glyph::Strength::Weak).unwrap();
    let edited =
        glyph::set_source_constraint_offset(&edited, 1, 12.0, Some(glyph::Unit::Px)).unwrap();
    assert!(edited.contains("constrain box.center_x >= canvas.center_x + 12px @weak;"));
    assert!(edited.contains("constrain box.left >= 10px @strong;"));
    let scene = parse_scene(&edited, "constraint-edit.glyph").unwrap();
    assert_eq!(scene.constraints.len(), 2);
    assert_eq!(scene.constraints[0].relation, glyph::Relation::Ge);
    assert_eq!(scene.constraints[0].strength, glyph::Strength::Weak);
    assert_eq!(scene.constraints[0].offset, 12.0);
    assert_eq!(scene.constraints[0].source_order, 1);
}

#[test]
fn constraint_source_remove_and_add_round_trip_deterministically() {
    let src = "scene \"Constraint Structure\" {\n  canvas 320 220;\n  rect box @id(\"box\") { x 20px; y 30px; width 80px; height 60px; }\n  constrain box.center_x == canvas.center_x @required;\n  constrain box.left >= 10px @strong;\n}\n";
    let removed = glyph::remove_source_constraint(src, 1).unwrap();
    assert!(!removed.contains("box.center_x == canvas.center_x"));
    assert!(removed.contains("constrain box.left >= 10px @strong;"));
    let reparsed = parse_scene(&removed, "constraint-remove.glyph").unwrap();
    assert_eq!(reparsed.constraints.len(), 1);
    assert_eq!(reparsed.constraints[0].id, "constraint.1");

    let added = glyph::add_source_constraint(
        &removed,
        &glyph::PropertyRef {
            node: "box".into(),
            property: "right".into(),
        },
        glyph::Relation::Le,
        &glyph::ConstraintRhs::Property(glyph::PropertyRef {
            node: "canvas".into(),
            property: "right".into(),
        }),
        -5.0,
        Some(glyph::Unit::Px),
        glyph::Strength::Medium,
    )
    .unwrap();
    assert!(added.contains("constrain box.right <= canvas.right - 5px @medium;"));
    let reparsed = parse_scene(&added, "constraint-add.glyph").unwrap();
    assert_eq!(reparsed.constraints.len(), 2);
    assert_eq!(reparsed.constraints[1].id, "constraint.2");
    assert_eq!(reparsed.constraints[1].offset, -5.0);
}

#[test]
fn constraint_source_edits_fail_closed_on_dimensional_mismatch() {
    let src = "scene \"Constraint Types\" {\n  canvas 320 220;\n  rect box @id(\"box\") { x 20px; y 30px; width 80px; height 60px; }\n  constrain box.left >= 10px @strong;\n}\n";
    let err =
        glyph::set_source_constraint_offset(src, 1, 90.0, Some(glyph::Unit::Deg)).unwrap_err();
    assert!(err.to_string().contains("dimension mismatch"), "{err}");

    let err = glyph::add_source_constraint(
        src,
        &glyph::PropertyRef {
            node: "box".into(),
            property: "left".into(),
        },
        glyph::Relation::Eq,
        &glyph::ConstraintRhs::Quantity(glyph::Quantity {
            value: 30.0,
            unit: glyph::Unit::Deg,
        }),
        0.0,
        None,
        glyph::Strength::Required,
    )
    .unwrap_err();
    assert!(err.to_string().contains("dimension mismatch"), "{err}");
}

#[test]
fn structural_groups_resolve_members_and_nested_bounds() {
    let src = r##"scene "Groups" {
        canvas 500 300;
        rect card @id("card") { x 20px; y 30px; width 100px; height 60px; }
        ellipse badge @id("badge") { cx 180px; cy 80px; rx 20px; ry 15px; }
        group inner @id("inner") { member card; }
        group outer @id("outer") { member inner; member badge; }
    }"##;
    let mut scene = parse_scene(src, "groups.glyph").unwrap();
    solve_scene(&mut scene);
    let inner = scene.nodes.iter().find(|node| node.id == "inner").unwrap();
    let outer = scene.nodes.iter().find(|node| node.id == "outer").unwrap();
    assert_eq!(inner.members, vec!["card"]);
    assert_eq!(outer.members, vec!["inner", "badge"]);
    assert_eq!(
        glyph::query::bounds_in_scene(&scene, inner),
        Some([20.0, 30.0, 100.0, 60.0])
    );
    assert_eq!(
        glyph::query::bounds_in_scene(&scene, outer),
        Some([20.0, 30.0, 180.0, 65.0])
    );
    let display = glyph::build_display_list(&scene);
    assert_eq!(
        display.object_map["outer"].bounds,
        Some([20.0, 30.0, 180.0, 65.0])
    );
}

#[test]
fn structural_group_membership_rejects_missing_duplicate_cycles_and_multiple_owners() {
    let missing = r#"scene "Missing" { canvas 100 100; group g { member nope; } }"#;
    assert!(
        parse_scene(missing, "group-missing.glyph")
            .unwrap_err()
            .to_string()
            .contains("missing member")
    );

    let duplicate = r#"scene "Duplicate" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; } group g { member a; member a; } }"#;
    assert!(
        parse_scene(duplicate, "group-duplicate.glyph")
            .unwrap_err()
            .to_string()
            .contains("duplicate member")
    );

    let cycle = r#"scene "Cycle" { canvas 100 100; group a { member b; } group b { member a; } }"#;
    assert!(
        parse_scene(cycle, "group-cycle.glyph")
            .unwrap_err()
            .to_string()
            .contains("membership cycle")
    );

    let owners = r#"scene "Owners" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; } group g1 { member a; } group g2 { member a; } }"#;
    assert!(
        parse_scene(owners, "group-owners.glyph")
            .unwrap_err()
            .to_string()
            .contains("multiple groups")
    );
}

#[test]
fn structural_group_drag_translates_members_and_preserves_internal_relations() {
    let src = r##"scene "Group Drag" {
        canvas 500 300;
        rect left @id("left") { x 40px; y 50px; width 80px; height 40px; fill "#111"; }
        rect right @id("right") { x 40px; y 120px; width 60px; height 30px; fill "#222"; }
        constrain left.x == right.x @required;
        group pair @id("pair") { member left; member right; }
    }"##;
    let mut scene = parse_scene(src, "group-drag.glyph").unwrap();
    solve_scene(&mut scene);
    let pair = scene.nodes.iter().find(|node| node.id == "pair").unwrap();
    assert_eq!(
        glyph::query::bounds_in_scene(&scene, pair),
        Some([40.0, 50.0, 80.0, 100.0])
    );

    let plan = glyph::plan_direct_manipulation(&scene, "pair", 25.0, -10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert_eq!(plan.member_plans.len(), 2);
    assert!(plan.constraint_edits.is_empty());
    let edited = glyph::apply_direct_manipulation_source(src, &plan).unwrap();
    assert!(edited.contains("member left;"), "{edited}");
    assert!(edited.contains("member right;"), "{edited}");
    assert!(edited.contains("x 65px;"), "{edited}");
    assert!(
        edited.contains("constrain left.x == right.x @required;"),
        "{edited}"
    );

    let mut reparsed = parse_scene(&edited, "group-drag.glyph").unwrap();
    solve_scene(&mut reparsed);
    let moved = reparsed
        .nodes
        .iter()
        .find(|node| node.id == "pair")
        .unwrap();
    assert_eq!(
        glyph::query::bounds_in_scene(&reparsed, moved),
        Some([65.0, 40.0, 80.0, 100.0])
    );
}

#[test]
fn component_path_resize_inverts_explicit_affine_parameters() {
    let src = r##"scene "Component Path Resize" {
        canvas 600 320;
        component icon {
            param sx: Number -> mark.scale_x = 1;
            param sy: Number -> mark.scale_y = 1;
            param tx: Length -> mark.translate_x = 0px;
            param ty: Length -> mark.translate_y = 0px;
            path mark @id("mark") {
                d "M10 20 L60 20 L60 70 Z";
                scale_x 1; scale_y 1; translate_x 0px; translate_y 0px; fill "#112233";
            }
        }
        instance icon icon1 @id("icon1") {
            x 100px; y 40px; scale 2;
            param sx 1; param sy 1; param tx 0px; param ty 0px;
        }
    }"##;
    let mut scene = parse_scene(src, "component-path-resize.glyph").unwrap();
    solve_scene(&mut scene);
    let before = scene
        .nodes
        .iter()
        .find(|node| node.id == "icon1/mark")
        .unwrap();
    assert_eq!(
        glyph::query::bounds(before),
        Some([120.0, 80.0, 100.0, 100.0])
    );

    let plan = glyph::plan_direct_resize(&scene, "icon1/mark", 20.0, 10.0).unwrap();
    assert!(plan.allowed, "{plan:?}");
    assert!(plan.edits.is_empty(), "{plan:?}");
    assert_eq!(plan.instance_parameter_edits.len(), 4, "{plan:?}");
    let edited = glyph::apply_direct_resize_source(src, &plan).unwrap();
    assert!(edited.contains("param sx 1.2;"), "{edited}");
    assert!(edited.contains("param sy 1.1;"), "{edited}");
    assert!(edited.contains("param tx -2px;"), "{edited}");
    assert!(edited.contains("param ty -2px;"), "{edited}");
    assert!(edited.contains("d \"M10 20 L60 20 L60 70 Z\";"), "{edited}");

    let mut reparsed = parse_scene(&edited, "component-path-resize.glyph").unwrap();
    solve_scene(&mut reparsed);
    let after = reparsed
        .nodes
        .iter()
        .find(|node| node.id == "icon1/mark")
        .unwrap();
    let bounds = glyph::query::bounds(after).unwrap();
    assert!((bounds[0] - 120.0).abs() < 0.0001, "{bounds:?}");
    assert!((bounds[1] - 80.0).abs() < 0.0001, "{bounds:?}");
    assert!((bounds[2] - 120.0).abs() < 0.0001, "{bounds:?}");
    assert!((bounds[3] - 110.0).abs() < 0.0001, "{bounds:?}");
}

#[test]
fn component_path_drag_inverts_translation_parameters_and_resize_requires_full_binding() {
    let drag_src = r##"scene "Component Path Drag" {
        canvas 600 320;
        component icon {
            param tx: Length -> mark.translate_x = 0px;
            param ty: Length -> mark.translate_y = 0px;
            path mark @id("mark") { d "M10 20 L60 20 L60 70 Z"; translate_x 0px; translate_y 0px; }
        }
        instance icon icon1 @id("icon1") { x 100px; y 40px; scale 2; param tx 0px; param ty 0px; }
    }"##;
    let mut drag_scene = parse_scene(drag_src, "component-path-drag.glyph").unwrap();
    solve_scene(&mut drag_scene);
    let before = drag_scene
        .nodes
        .iter()
        .find(|node| node.id == "icon1/mark")
        .unwrap();
    assert_eq!(
        glyph::query::bounds(before),
        Some([120.0, 80.0, 100.0, 100.0])
    );
    let drag = glyph::plan_direct_manipulation(&drag_scene, "icon1/mark", 20.0, -10.0).unwrap();
    assert!(drag.allowed, "{drag:?}");
    assert_eq!(drag.instance_parameter_edits.len(), 2);
    let dragged = glyph::apply_direct_manipulation_source(drag_src, &drag).unwrap();
    assert!(dragged.contains("param tx 10px;"), "{dragged}");
    assert!(dragged.contains("param ty -5px;"), "{dragged}");
    let mut reparsed = parse_scene(&dragged, "component-path-drag.glyph").unwrap();
    solve_scene(&mut reparsed);
    let after = reparsed
        .nodes
        .iter()
        .find(|node| node.id == "icon1/mark")
        .unwrap();
    assert_eq!(
        glyph::query::bounds(after),
        Some([140.0, 70.0, 100.0, 100.0])
    );

    let resize = glyph::plan_direct_resize(&drag_scene, "icon1/mark", 20.0, 0.0).unwrap();
    assert!(!resize.allowed, "{resize:?}");
    assert!(
        resize
            .blocked
            .iter()
            .any(|block| block.reason.contains("scale_x")),
        "{resize:?}"
    );
}

#[test]
fn accessibility_semantics_lower_to_display_svg_and_commander_draw() {
    let src = r##"scene "Accessibility" {
        canvas 320 180;
        text label @id("label") { content "Continue"; x 20px; y 40px; size 18px; a11y_hidden "true"; }
        rect button @id("button") {
            x 20px; y 60px; width 120px; height 50px; fill "#224466";
            a11y_role "button"; a11y_labelled_by "label"; a11y_hint "Continues to checkout";
            interactive "true"; reading_order 1;
        }
    }"##;
    let mut scene = parse_scene(src, "accessibility.glyph").unwrap();
    solve_scene(&mut scene);
    let display = glyph::build_display_list(&scene);
    let button = &display.object_map["button"].accessibility;
    assert_eq!(button.role.as_deref(), Some("button"));
    assert_eq!(button.labelled_by.as_deref(), Some("label"));
    assert_eq!(button.hint.as_deref(), Some("Continues to checkout"));
    assert!(button.interactive);
    assert_eq!(button.reading_order, Some(1));
    let svg = glyph::render_svg(&scene);
    assert!(svg.contains("role=\"button\""), "{svg}");
    assert!(svg.contains("aria-labelledby=\"label\""), "{svg}");
    assert!(
        svg.contains("aria-description=\"Continues to checkout\""),
        "{svg}"
    );
    assert!(svg.contains("tabindex=\"0\""), "{svg}");
    assert!(svg.contains("data-glyph-reading-order=\"1\""), "{svg}");
    assert!(svg.contains("aria-hidden=\"true\""), "{svg}");

    let draw = glyph::commander_draw_document(&scene);
    assert_eq!(
        draw["nodes"][0]["glyphAccessibility"]["hidden"],
        serde_json::json!(true)
    );
    assert_eq!(
        draw["nodes"][1]["glyphAccessibility"]["labelledBy"],
        serde_json::json!("label")
    );
}

#[test]
fn accessibility_audit_reports_structural_failures_deterministically() {
    let src = r##"scene "Accessibility Audit" {
        canvas 320 220;
        rect first @id("first") { x 10px; y 10px; width 30px; height 30px; interactive "true"; reading_order 1; }
        rect second @id("second") { x 60px; y 10px; width 60px; height 50px; a11y_role "button"; reading_order 1; }
        rect hidden @id("hidden") { x 10px; y 80px; width 60px; height 50px; a11y_hidden "true"; interactive "true"; a11y_label "Hidden action"; }
        rect broken @id("broken") { x 100px; y 80px; width 60px; height 50px; a11y_labelled_by "missing"; }
    }"##;
    let mut scene = parse_scene(src, "accessibility-audit.glyph").unwrap();
    solve_scene(&mut scene);
    let report = glyph::accessibility_report(&scene);
    assert_eq!(report.interactive_nodes, 1);
    assert_eq!(report.reading_order[..2], ["first", "second"]);
    let codes = report
        .issues
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"missing_accessible_name"), "{report:?}");
    assert!(codes.contains(&"small_interactive_target"), "{report:?}");
    assert!(codes.contains(&"duplicate_reading_order"), "{report:?}");
    assert!(codes.contains(&"hidden_interactive"), "{report:?}");
    assert!(codes.contains(&"missing_label_target"), "{report:?}");
}

#[test]
fn accessibility_properties_fail_closed_on_invalid_values() {
    let bad_flag = r#"scene "Bad Flag" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; interactive "yes"; } }"#;
    let err = parse_scene(bad_flag, "bad-a11y-flag.glyph").unwrap_err();
    assert!(err.to_string().contains("expects true or false"), "{err}");

    let bad_live = r#"scene "Bad Live" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; a11y_live "loud"; } }"#;
    let err = parse_scene(bad_live, "bad-a11y-live.glyph").unwrap_err();
    assert!(
        err.to_string().contains("off, polite, or assertive"),
        "{err}"
    );

    let bad_order = r#"scene "Bad Order" { canvas 100 100; rect a { x 0; y 0; width 10; height 10; reading_order 1.5; } }"#;
    let err = parse_scene(bad_order, "bad-reading-order.glyph").unwrap_err();
    assert!(err.to_string().contains("non-negative integer"), "{err}");
}

#[test]
fn language_diagnostics_report_exact_parse_location_and_semantic_warnings() {
    let broken = "scene \"Broken\" {\n  canvas 100 100\n}\n";
    let diagnostics = glyph::diagnose_source(broken, "broken.glyph");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "glyph.parse");
    assert_eq!(diagnostics[0].range.start.line, 1);
    assert_eq!(diagnostics[0].range.start.character, 13);

    let source = r#"scene "Tooling" {
        canvas 200 120;
        rect action @id("action") { x 0; y 0; width 20; height 20; interactive "true"; }
    }"#;
    let diagnostics = glyph::diagnose_source(source, "tooling.glyph");
    assert!(
        diagnostics
            .iter()
            .any(|item| { item.code == "glyph.accessibility.missing_accessible_name" }),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| { item.code == "glyph.accessibility.small_interactive_target" }),
        "{diagnostics:?}"
    );
}

#[test]
fn language_completion_is_context_aware_for_nodes_groups_and_constraints() {
    let source = "scene \"Completion\" {\n  canvas 300 200;\n  rect card {\n    \n  }\n  group stack {\n    member \n  }\n  constrain \n}\n";
    let node_items = glyph::complete_source(
        source,
        glyph::TextPosition {
            line: 3,
            character: 4,
        },
    );
    assert!(
        node_items.iter().any(|item| item.label == "width"),
        "{node_items:?}"
    );
    assert!(
        node_items.iter().any(|item| item.label == "a11y_label"),
        "{node_items:?}"
    );

    let member_items = glyph::complete_source(
        source,
        glyph::TextPosition {
            line: 6,
            character: 11,
        },
    );
    assert!(
        member_items.iter().any(|item| item.label == "card"),
        "{member_items:?}"
    );

    let constraint_items = glyph::complete_source(
        source,
        glyph::TextPosition {
            line: 8,
            character: 12,
        },
    );
    assert!(
        constraint_items.iter().any(|item| item.label == "canvas"),
        "{constraint_items:?}"
    );
    assert!(
        constraint_items.iter().any(|item| item.label == "card"),
        "{constraint_items:?}"
    );
}

#[test]
fn lsp_stdio_handshake_publishes_diagnostics_and_returns_completions() {
    fn frame(value: serde_json::Value) -> Vec<u8> {
        let body = serde_json::to_vec(&value).unwrap();
        let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        framed.extend(body);
        framed
    }
    let source = "scene \"LSP\" {\n  canvas 200 120;\n  rect card {\n    \n    x 20px; y 20px; width 80px; height 60px; interactive true; a11y_role button;\n  }\n  group pair { member card; }\n  constrain card.left == canvas.left + 20px @required;\n}\n";
    let uri = "file:///tmp/lsp-test.glyph";
    let messages = [
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":"glyph","version":1,"text":source}}}),
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":uri},"position":{"line":3,"character":4}}}),
        serde_json::json!({"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":uri},"position":{"line":7,"character":13}}}),
        serde_json::json!({"jsonrpc":"2.0","id":4,"method":"textDocument/references","params":{"textDocument":{"uri":uri},"position":{"line":7,"character":13},"context":{"includeDeclaration":true}}}),
        serde_json::json!({"jsonrpc":"2.0","id":5,"method":"textDocument/prepareRename","params":{"textDocument":{"uri":uri},"position":{"line":7,"character":13}}}),
        serde_json::json!({"jsonrpc":"2.0","id":6,"method":"textDocument/rename","params":{"textDocument":{"uri":uri},"position":{"line":7,"character":13},"newName":"panel"}}),
        serde_json::json!({"jsonrpc":"2.0","id":7,"method":"textDocument/codeAction","params":{"textDocument":{"uri":uri},"range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"context":{"diagnostics":[{"range":{"start":{"line":2,"character":0},"end":{"line":2,"character":1}},"code":"glyph.accessibility.missing_accessible_name","message":"missing name"}]}}}),
        serde_json::json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":uri,"version":2},"contentChanges":[{"range":{"start":{"line":3,"character":4},"end":{"line":3,"character":4}},"text":"width 90px;"}]}}),
        serde_json::json!({"jsonrpc":"2.0","id":8,"method":"shutdown","params":null}),
        serde_json::json!({"jsonrpc":"2.0","method":"exit","params":null}),
    ];
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_glyph"))
        .arg("lsp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        for message in messages {
            stdin.write_all(&frame(message)).unwrap();
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.status);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("glyph-lsp"), "{stdout}");
    assert!(
        stdout.contains("textDocument/publishDiagnostics"),
        "{stdout}"
    );
    assert!(stdout.contains("\"textDocumentSync\":2"), "{stdout}");
    assert!(stdout.contains("\"definitionProvider\":true"), "{stdout}");
    assert!(stdout.contains("\"referencesProvider\":true"), "{stdout}");
    assert!(stdout.contains("\"label\":\"width\""), "{stdout}");
    assert!(stdout.contains("\"placeholder\":\"card\""), "{stdout}");
    assert!(stdout.contains("\"newText\":\"panel\""), "{stdout}");
    assert!(stdout.contains("Add accessible name to card"), "{stdout}");
    assert!(
        stdout.contains("\"id\":8,\"jsonrpc\":\"2.0\",\"result\":null"),
        "{stdout}"
    );
}

#[test]
fn project_language_diagnostics_resolve_imports_with_unsaved_entry_overlay() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("glyph.project.json"), r#"{"version":1}"#).unwrap();
    fs::write(
        dir.path().join("library.glyph"),
        r##"scene "Library" { canvas 10 10; style accent { fill "#112233"; } }"##,
    )
    .unwrap();
    let entry = dir.path().join("main.glyph");
    fs::write(
        &entry,
        r#"import "library.glyph" as ui; scene "Main" { canvas 200 120; }"#,
    )
    .unwrap();
    let overlay = r#"import "library.glyph" as ui;
scene "Main" {
  canvas 200 120;
  rect card @id("card") { use ui/accent; x 20px; y 20px; width 80px; height 50px; }
}"#;
    let diagnostics = glyph::diagnose_project_source(&entry, overlay);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let scene = glyph::load_project_scene_with_source(&entry, overlay).unwrap();
    let card = scene.nodes.iter().find(|node| node.id == "card").unwrap();
    assert_eq!(card.authored.props["fill"].text(), Some("#112233"));
}

#[test]
fn language_navigation_finds_definition_and_document_references() {
    let src = r##"scene "Navigation" {
  canvas 400 240;
  rect box @id("stable-box") { x 20px; y 20px; width 80px; height 60px; }
  group pair @id("pair") { member box; }
  constrain box.left == canvas.left + 20px @required;
  state active { set box.fill "#ff0000"; }
  component card {
    rect box @id("local-box") { x 0px; y 0px; width 20px; height 20px; }
    param inset: Length -> box.x = 0px;
  }
}"##;
    let position = glyph::TextPosition {
        line: 4,
        character: 13,
    };
    let refs = glyph::references_at(src, position, true);
    assert_eq!(refs.len(), 4, "{refs:?}");
    assert_eq!(
        refs.iter()
            .filter(|reference| reference.declaration)
            .count(),
        1
    );
    let definition = glyph::definition_at(src, position).expect("definition");
    assert_eq!(definition.range.start.line, 2);
    assert!(refs.iter().all(|reference| reference.range.start.line < 7));
}

#[test]
fn language_rename_preserves_stable_ids_and_component_local_scope() {
    let src = r##"scene "Rename" {
  canvas 400 240;
  rect box @id("stable-box") { x 20px; y 20px; width 80px; height 60px; }
  group pair @id("pair") { member box; }
  constrain box.left == canvas.left + 20px @required;
  state active { set box.fill "#ff0000"; }
  component card {
    rect box @id("local-box") { x 0px; y 0px; width 20px; height 20px; }
    param inset: Length -> box.x = 0px;
  }
}"##;
    let position = glyph::TextPosition {
        line: 4,
        character: 13,
    };
    let plan = glyph::rename_node_at(src, position, "hero").unwrap();
    assert_eq!(plan.symbol, "box");
    assert_eq!(plan.edits.len(), 4);
    let edited = glyph::apply_source_text_edits(src, &plan.edits).unwrap();
    assert!(edited.contains("rect hero @id(\"stable-box\")"), "{edited}");
    assert!(edited.contains("member hero;"), "{edited}");
    assert!(edited.contains("constrain hero.left"), "{edited}");
    assert!(edited.contains("set hero.fill"), "{edited}");
    assert!(edited.contains("rect box @id(\"local-box\")"), "{edited}");
    assert!(edited.contains("-> box.x = 0px;"), "{edited}");
    parse_scene(&edited, "renamed.glyph").unwrap();

    let conflict = glyph::rename_node_at(src, position, "pair").unwrap_err();
    assert!(conflict.contains("already exists"), "{conflict}");
    let reserved = glyph::rename_node_at(src, position, "rect").unwrap_err();
    assert!(
        reserved.contains("safe Glyph node identifier"),
        "{reserved}"
    );
}

#[test]
fn language_code_action_fixes_missing_accessible_name() {
    let src = r##"scene "A11y Fix" {
  canvas 200 120;
  rect submit_button @id("submit") { x 20px; y 20px; width 60px; height 60px; interactive true; a11y_role button; }
}"##;
    let diagnostics = glyph::diagnose_source(src, "a11y-fix.glyph");
    let missing = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "glyph.accessibility.missing_accessible_name")
        .expect("missing-name diagnostic");
    let actions = glyph::code_actions(src, std::slice::from_ref(missing));
    assert_eq!(actions.len(), 1, "{actions:?}");
    let edited = glyph::apply_source_text_edits(src, &actions[0].edits).unwrap();
    assert!(edited.contains("a11y_label \"submit button\";"), "{edited}");
    let after = glyph::diagnose_source(&edited, "a11y-fix.glyph");
    assert!(
        !after
            .iter()
            .any(|diagnostic| diagnostic.code == "glyph.accessibility.missing_accessible_name")
    );
}

#[test]
fn source_group_create_add_remove_and_ungroup_round_trip() {
    let src = r##"scene "Hierarchy" {
  canvas 400 260;
  rect first @id("first-id") { x 20px; y 30px; width 80px; height 40px; }
  rect second @id("second-id") { x 140px; y 50px; width 60px; height 50px; }
  rect third @id("third-id") { x 240px; y 80px; width 40px; height 30px; }
}"##;
    let members = vec!["first-id".to_string(), "second-id".to_string()];
    let (grouped, group_id) = glyph::create_source_group(src, None, &members).unwrap();
    assert_eq!(group_id, "group");
    assert!(grouped.contains("group group {"), "{grouped}");
    assert!(grouped.contains("member first-id;"), "{grouped}");
    assert!(grouped.contains("member second-id;"), "{grouped}");
    let mut scene = parse_scene(&grouped, "hierarchy.glyph").unwrap();
    solve_scene(&mut scene);
    let group = scene.nodes.iter().find(|node| node.id == "group").unwrap();
    assert_eq!(group.members, vec!["first-id", "second-id"]);
    assert_eq!(
        glyph::query::bounds_in_scene(&scene, group),
        Some([20.0, 30.0, 180.0, 70.0])
    );

    let expanded = glyph::add_source_group_member(&grouped, "group", "third-id").unwrap();
    let scene = parse_scene(&expanded, "hierarchy-expanded.glyph").unwrap();
    assert_eq!(
        scene
            .nodes
            .iter()
            .find(|node| node.id == "group")
            .unwrap()
            .members,
        vec!["first-id", "second-id", "third-id"]
    );

    let reduced = glyph::remove_source_group_member(&expanded, "group", "second-id").unwrap();
    let scene = parse_scene(&reduced, "hierarchy-reduced.glyph").unwrap();
    assert_eq!(
        scene
            .nodes
            .iter()
            .find(|node| node.id == "group")
            .unwrap()
            .members,
        vec!["first-id", "third-id"]
    );

    let ungrouped = glyph::remove_source_group(&reduced, "group").unwrap();
    let scene = parse_scene(&ungrouped, "hierarchy-ungrouped.glyph").unwrap();
    assert!(scene.nodes.iter().all(|node| node.id != "group"));
    assert!(scene.nodes.iter().any(|node| node.id == "first-id"));
    assert!(scene.nodes.iter().any(|node| node.id == "third-id"));
}

#[test]
fn source_group_edits_fail_closed_on_ownership_and_parent_dependencies() {
    let src = r##"scene "Hierarchy Guards" {
  canvas 320 220;
  rect a @id("a") { x 20px; y 20px; width 40px; height 40px; }
  rect b @id("b") { x 80px; y 20px; width 40px; height 40px; }
  rect c @id("c") { x 140px; y 20px; width 40px; height 40px; }
  group inner @id("inner") { member a; member b; }
  group outer @id("outer") { member inner; member c; }
}"##;
    let owned = glyph::add_source_group_member(src, "outer", "a").unwrap_err();
    assert!(owned.to_string().contains("already belongs"), "{owned}");

    let duplicate =
        glyph::create_source_group(src, Some("new_group"), &["c".to_string(), "c".to_string()])
            .unwrap_err();
    assert!(
        duplicate.to_string().contains("selected more than once")
            || duplicate.to_string().contains("already belongs"),
        "{duplicate}"
    );

    let parent = glyph::remove_source_group(src, "inner").unwrap_err();
    assert!(
        parent.to_string().contains("parent group outer"),
        "{parent}"
    );

    let invalid_name =
        glyph::create_source_group(src, Some("bad/name"), &["a".to_string(), "b".to_string()])
            .unwrap_err();
    assert!(
        invalid_name.to_string().contains("safe top-level"),
        "{invalid_name}"
    );
}
