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
