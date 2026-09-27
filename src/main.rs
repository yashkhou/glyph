use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use glyph::{
    ConstraintRhs, NodePlacement, PatchOperation, PropertyRef, Relation, ResizeHandle, Scene,
    Strength, Unit, Value, accessibility_report, add_source_constraint, add_source_group_member,
    apply_direct_handle_resize_source, apply_direct_manipulation_source,
    apply_direct_resize_source, apply_transaction, build_display_list, create_source_group,
    diff::diff_scenes,
    direct_source_nodes, evaluate_assertions, load_project_scene, move_source_node,
    plan_direct_manipulation, plan_direct_resize, plan_direct_resize_from_handle, query,
    remove_source_constraint, remove_source_group, remove_source_group_member, render_svg,
    render_swiftui, render_web_canvas, renderer_conformance_report, sample_motion, scene_in_state,
    set_source_constraint_offset, set_source_constraint_relation, set_source_constraint_strength,
    set_source_property, solve_scene, target_strategy_report, validate_project,
    vision::{VisionImportMode, default_vision_helper, import_image_to_glyph_with_mode},
    write_project_bundle,
};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "glyph",
    version,
    about = "Semantic design compiler and agent-native visual runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Capabilities,
    Check {
        input: PathBuf,
    },
    ProjectCheck {
        input: PathBuf,
    },
    ProjectBundle {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Compile {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Render {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    ExportDraw {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    RenderPng {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "/Users/joshuakhoury/Projects/CommanderDraw")]
        commander_draw: PathBuf,
    },
    ExportPdf {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "/Users/joshuakhoury/Projects/CommanderDraw")]
        commander_draw: PathBuf,
    },
    ExportSwiftui {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    ExportWebCanvas {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    ImportImage {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        vision_helper: Option<PathBuf>,
        #[arg(long)]
        manifest_output: Option<PathBuf>,
        #[arg(long, default_value = "auto")]
        mode: String,
    },
    Inspect {
        input: PathBuf,
        #[arg(long)]
        node: Option<String>,
    },
    Query {
        input: PathBuf,
        #[arg(long)]
        role: String,
    },
    Why {
        input: PathBuf,
        node: String,
    },
    Patch {
        input: PathBuf,
        patch: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Apply {
        input: PathBuf,
        base_revision: u64,
        operations: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    Test {
        input: PathBuf,
    },
    State {
        input: PathBuf,
        name: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    MotionSample {
        input: PathBuf,
        time: f64,
        #[arg(short, long)]
        output: PathBuf,
    },
    SourceSet {
        input: PathBuf,
        node: String,
        property: String,
        value: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    Constraints {
        input: PathBuf,
        #[arg(long)]
        node: Option<String>,
    },
    ConstraintEdit {
        input: PathBuf,
        source_order: usize,
        #[arg(long)]
        relation: Option<String>,
        #[arg(long)]
        strength: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        offset: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
    },
    ConstraintRemove {
        input: PathBuf,
        source_order: usize,
        #[arg(short, long)]
        output: PathBuf,
    },
    ConstraintAdd {
        input: PathBuf,
        lhs: String,
        relation: String,
        rhs: String,
        #[arg(long, allow_hyphen_values = true)]
        offset: Option<String>,
        #[arg(long, default_value = "required")]
        strength: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    GroupCreate {
        input: PathBuf,
        members: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(short, long)]
        output: PathBuf,
    },
    GroupAddMember {
        input: PathBuf,
        group: String,
        member: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    GroupRemoveMember {
        input: PathBuf,
        group: String,
        member: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    GroupRemove {
        input: PathBuf,
        group: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    Hierarchy {
        input: PathBuf,
    },
    NodeMove {
        input: PathBuf,
        node: String,
        anchor: String,
        placement: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    DragPlan {
        input: PathBuf,
        node: String,
        #[arg(allow_hyphen_values = true)]
        dx: f64,
        #[arg(allow_hyphen_values = true)]
        dy: f64,
    },
    SourceDrag {
        input: PathBuf,
        node: String,
        #[arg(allow_hyphen_values = true)]
        dx: f64,
        #[arg(allow_hyphen_values = true)]
        dy: f64,
        #[arg(short, long)]
        output: PathBuf,
    },
    ResizePlan {
        input: PathBuf,
        node: String,
        #[arg(allow_hyphen_values = true)]
        dw: f64,
        #[arg(allow_hyphen_values = true)]
        dh: f64,
    },
    SourceResize {
        input: PathBuf,
        node: String,
        #[arg(allow_hyphen_values = true)]
        dw: f64,
        #[arg(allow_hyphen_values = true)]
        dh: f64,
        #[arg(short, long)]
        output: PathBuf,
    },
    ResizeHandlePlan {
        input: PathBuf,
        node: String,
        handle: String,
        #[arg(allow_hyphen_values = true)]
        dx: f64,
        #[arg(allow_hyphen_values = true)]
        dy: f64,
    },
    SourceResizeHandle {
        input: PathBuf,
        node: String,
        handle: String,
        #[arg(allow_hyphen_values = true)]
        dx: f64,
        #[arg(allow_hyphen_values = true)]
        dy: f64,
        #[arg(short, long)]
        output: PathBuf,
    },
    DisplayList {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    ObjectMap {
        input: PathBuf,
    },
    Conformance {
        input: PathBuf,
    },
    Accessibility {
        input: PathBuf,
    },
    Diagnostics {
        input: PathBuf,
    },
    Complete {
        input: PathBuf,
        line: u32,
        character: u32,
    },
    Lsp,
    Targets {
        input: PathBuf,
    },
    Diff {
        before: PathBuf,
        after: PathBuf,
    },
    VisualDiff {
        before: PathBuf,
        after: PathBuf,
    },
}

#[derive(Debug, Deserialize)]
struct PatchBundle {
    base_revision: u64,
    operations: Vec<PatchOperation>,
}

fn parse_file(path: &Path) -> Result<Scene> {
    let mut scene = load_project_scene(path)?;
    let report = solve_scene(&mut scene);
    if !report.required_conflicts.is_empty() || !report.unresolved.is_empty() {
        anyhow::bail!(
            "Glyph solve failed: {} required conflict(s), {} unresolved constraint(s)",
            report.required_conflicts.len(),
            report.unresolved.len()
        );
    }
    Ok(scene)
}

fn load_gsir(path: &Path) -> Result<Scene> {
    let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(glyph::load_gsir_str(&source)?.0)
}

fn load_scene(path: &Path) -> Result<Scene> {
    if path.extension().and_then(|x| x.to_str()) == Some("glyph") {
        parse_file(path)
    } else {
        load_gsir(path)
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_property_ref_arg(raw: &str) -> Result<PropertyRef> {
    let (node, property) = raw
        .split_once('.')
        .ok_or_else(|| anyhow!("property reference must be node.property, got {raw:?}"))?;
    if node.is_empty() || property.is_empty() {
        return Err(anyhow!(
            "property reference must be node.property, got {raw:?}"
        ));
    }
    Ok(PropertyRef {
        node: node.to_string(),
        property: property.to_string(),
    })
}

fn parse_relation_arg(raw: &str) -> Result<Relation> {
    match raw {
        "eq" | "==" => Ok(Relation::Eq),
        "ge" | ">=" => Ok(Relation::Ge),
        "le" | "<=" => Ok(Relation::Le),
        _ => Err(anyhow!(
            "relation must be eq, ge, le, ==, >=, or <=; got {raw:?}"
        )),
    }
}

fn parse_strength_arg(raw: &str) -> Result<Strength> {
    match raw.trim_start_matches('@') {
        "required" => Ok(Strength::Required),
        "strong" => Ok(Strength::Strong),
        "medium" => Ok(Strength::Medium),
        "weak" => Ok(Strength::Weak),
        _ => Err(anyhow!(
            "strength must be required, strong, medium, or weak; got {raw:?}"
        )),
    }
}

fn parse_constraint_rhs_arg(raw: &str) -> Result<ConstraintRhs> {
    if raw.contains('.') {
        return Ok(ConstraintRhs::Property(parse_property_ref_arg(raw)?));
    }
    match glyph::parse_value_literal(raw)? {
        Value::Number(value) => Ok(ConstraintRhs::Number(value)),
        Value::Quantity(quantity) => Ok(ConstraintRhs::Quantity(quantity)),
        Value::Text(_) => Err(anyhow!(
            "constraint rhs must be node.property, a number, or a typed quantity; got {raw:?}"
        )),
    }
}

fn parse_constraint_offset_arg(raw: &str) -> Result<(f64, Option<Unit>)> {
    match glyph::parse_value_literal(raw)? {
        Value::Number(value) => Ok((value, None)),
        Value::Quantity(quantity) => Ok((quantity.canonical_value(), Some(quantity.unit))),
        Value::Text(_) => Err(anyhow!(
            "constraint offset must be a number or typed quantity; got {raw:?}"
        )),
    }
}

fn require_glyph_source(path: &Path, operation: &str) -> Result<()> {
    if path.extension().and_then(|value| value.to_str()) != Some("glyph") {
        return Err(anyhow!("{operation} requires a .glyph source input"));
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capabilities => print_json(&serde_json::json!({
            "name":"Glyph Core", "version":env!("CARGO_PKG_VERSION"),
            "canonical":"GSIR", "sourceExtension":".glyph",
            "versions":{"source":glyph::CURRENT_SOURCE_VERSION,"gsirSchema":glyph::CURRENT_GSIR_SCHEMA_VERSION},
            "units":["px","deg","rad","%","s","ms"],
            "nodeKinds":["rect","ellipse","path","text","image","group"],
            "constraints":["==",">=","<=","required","strong","medium","weak"],
            "reuse":["styles","typed-linear-radial-paints","components","instances","seeded-repeat","local-project-imports"],
            "runtime":["assertions","named-states","sampled-motion","motion-delay-repeat-alternate","deterministic-spring-presets","deterministic-text-layout"],
            "agent":["query-by-role","inspect","why","constraint-query","source-backed-constraint-editing","revision-safe-apply","semantic-diff","visual-diff","source-round-trip","constraint-aware-direct-manipulation","source-backed-drag-plan","constraint-aware-direct-resize","source-backed-resize-plan","multi-edge-resize","structural-group-translation","source-hierarchy-query","source-node-reorder","language-diagnostics","context-completion","language-definition","language-references","safe-node-rename","language-code-actions","lsp-stdio","display-list","object-map","renderer-conformance","accessibility-audit","target-strategy","bounded-image-import","project-check","deterministic-project-bundle","semantic-swiftui-codegen","semantic-web-canvas-codegen"],
            "outputs":["glyph-source","gsir-json","display-list-json","object-map-json","visual-diff-json","language-diagnostics-json","renderer-conformance-json","accessibility-audit-json","target-strategy-json","project-bundle-directory","project-bundle-manifest-json","svg","png-via-CommanderDraw","pdf-via-CommanderDraw","commander-draw-json","swiftui-source","web-canvas-module"],
            "guarantees":["stable semantic IDs","authored/derived separation","transaction revision checks","deterministic SVG","deterministic display list","seeded procedural determinism","Vision import confidence/provenance retained in source","project assets content-addressed by SHA-256","project bundles preserve transitive source paths and verify before publish"],
            "status":{"vision":"Apple Vision import v1 supports structured editable UI reconstruction plus deterministic auto/hybrid fallback that preserves ambiguous photo/illustration pixels as a raster base with non-rendering confidence-bearing semantic overlays; not arbitrary photo-to-clean-vector reconstruction","motion":"deterministic sampled timeline v1 with typed time units, delay, finite repeat, alternate playback and spring/ease presets; not a continuous physics engine","studio":"native macOS Studio v0.1: source + resolved plate + semantic ledger + intent inspector + source-backed edits + constraint-aware direct drag/resize + first-class constraint badges/edit/add/remove + validated source undo/redo; semantic text-frame resize, explicit multi-edge/corner handles, structural group translation/grouping, direct source/render-order reordering for authored nodes and whole component-instance owners, compatible equality co-editing, inequality guards, unambiguous component inverse edits, affine path resize with supported translate constraints, and explicit component-owned path affine bindings are implemented; unsupported inverse intent fails closed","solver":"Cassowary-class weighted linear solver v1; scene solve rebuilds solver per invocation","text":"macOS CoreText shaping/metrics/wrapping with fallback/substitution diagnostics, letter spacing, ligature policy, and validated OpenType variable-font axes; deterministic non-macOS fallback only; CommanderDraw preserves advanced typography metadata but exact backend fidelity is not guaranteed","roundTrip":"semantic property + first-class constraint relation/strength/offset/add/remove + structural grouping + direct layer-owner source/render-order edits v1","displayList":"backend-neutral display list + object map v3; typed paints, affine transforms, resolved clip/mask references, shaped text metrics","conformance":"machine-readable SVG/CommanderDraw exact/degraded/unsupported backend contract; pixel fidelity remains separately verified","accessibility":"first-class a11y labels/roles/hints/label references/live regions/reading order/interactive metadata lower through display-list, SVG ARIA and CommanderDraw metadata; deterministic audit flags missing names/targets, duplicate reading order, hidden interactivity and undersized targets","tooling":"deterministic parser/solver/accessibility diagnostics, context-aware source completions, definition/references, fail-closed top-level node rename, accessibility quick fixes, and stdio LSP v1 with UTF-16-safe incremental synchronization; project diagnostics resolve local imports while overlaying unsaved entry source; project manifests verify SHA-256 assets and publish validated self-contained dependency bundles","targets":"visual export and semantic codegen are separate contracts; PDF is implemented via CommanderDraw; SwiftUI and Web Canvas emitters are implemented for a documented bounded display-list subset and fail closed on unsupported semantics","visualDiff":"stable-ID resolved visual diff v1"}
        })),
        Command::Check { input } => {
            let mut scene = load_project_scene(&input)?;
            let report = solve_scene(&mut scene);
            let ok = report.required_conflicts.is_empty() && report.unresolved.is_empty();
            print_json(
                &serde_json::json!({"ok":ok,"scene":scene.id,"revision":scene.revision,"nodes":scene.nodes.len(),"constraints":scene.constraints.len(),"solve":report}),
            )?;
            if !ok {
                anyhow::bail!("Glyph check failed: unresolved or conflicting required constraints");
            }
            Ok(())
        }
        Command::ProjectCheck { input } => {
            let report = validate_project(&input)?;
            print_json(&report)
        }
        Command::ProjectBundle { input, output } => {
            let bundle = write_project_bundle(&input, &output)?;
            print_json(&serde_json::json!({
                "ok": true,
                "output": output,
                "bundle": bundle
            }))
        }
        Command::Compile { input, output } => {
            let scene = parse_file(&input)?;
            fs::write(&output, serde_json::to_string_pretty(&scene)? + "\n")?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"scene":scene.id,"revision":scene.revision,"nodes":scene.nodes.len()}),
            )
        }
        Command::Render { input, output } => {
            let scene = load_scene(&input)?;
            fs::write(&output, render_svg(&scene))?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"svg","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::ExportDraw { input, output } => {
            let scene = load_scene(&input)?;
            glyph::bridge::write_commander_draw(&scene, &output)?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"commander-draw","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::RenderPng {
            input,
            output,
            commander_draw,
        } => {
            let scene = load_scene(&input)?;
            glyph::bridge::render_png_with_commander_draw(&scene, &output, &commander_draw)?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"png","backend":"CommanderDraw","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::ExportPdf {
            input,
            output,
            commander_draw,
        } => {
            let scene = load_scene(&input)?;
            glyph::bridge::export_pdf_with_commander_draw(&scene, &output, &commander_draw)?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"pdf","backend":"CommanderDraw","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::ExportSwiftui { input, output } => {
            let scene = load_scene(&input)?;
            fs::write(&output, render_swiftui(&scene)?)?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"swiftui","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::ExportWebCanvas { input, output } => {
            let scene = load_scene(&input)?;
            fs::write(&output, render_web_canvas(&scene)?)?;
            print_json(
                &serde_json::json!({"ok":true,"output":output,"format":"web_canvas","scene":scene.id,"revision":scene.revision}),
            )
        }
        Command::ImportImage {
            input,
            output,
            vision_helper,
            manifest_output,
            mode,
        } => {
            let helper = vision_helper.unwrap_or_else(default_vision_helper);
            let mode = mode
                .parse::<VisionImportMode>()
                .map_err(anyhow::Error::msg)?;
            let summary = import_image_to_glyph_with_mode(
                &input,
                &output,
                &helper,
                manifest_output.as_deref(),
                mode,
            )?;
            print_json(&summary)
        }
        Command::Inspect { input, node } => {
            let scene = load_scene(&input)?;
            if let Some(id) = node {
                let found = scene
                    .nodes
                    .iter()
                    .find(|n| n.id == id || n.name == id)
                    .context("node not found")?;
                print_json(found)
            } else {
                print_json(&scene)
            }
        }
        Command::Query { input, role } => {
            let scene = load_scene(&input)?;
            print_json(&query::by_role(&scene, &role))
        }
        Command::Why { input, node } => {
            let scene = load_scene(&input)?;
            let found = scene
                .nodes
                .iter()
                .find(|n| n.id == node || n.name == node)
                .context("node not found")?;
            let constraints = query::constraints_for(&scene, &found.id);
            print_json(&serde_json::json!({
                "node": found.id,
                "authored": found.authored,
                "derived": found.derived,
                "provenance": found.provenance,
                "constraints": constraints
            }))
        }
        Command::Patch {
            input,
            patch,
            output,
        } => {
            let mut scene = load_scene(&input)?;
            let bundle: PatchBundle = serde_json::from_str(&fs::read_to_string(&patch)?)?;
            let result = apply_transaction(&mut scene, bundle.base_revision, &bundle.operations)?;
            fs::write(&output, serde_json::to_string_pretty(&scene)? + "\n")?;
            print_json(&result)
        }
        Command::Apply {
            input,
            base_revision,
            operations,
            output,
        } => {
            let mut scene = load_scene(&input)?;
            let operations: Vec<PatchOperation> = serde_json::from_str(&operations)?;
            let result = apply_transaction(&mut scene, base_revision, &operations)?;
            fs::write(&output, serde_json::to_string_pretty(&scene)? + "\n")?;
            print_json(&result)
        }
        Command::Test { input } => {
            let scene = load_scene(&input)?;
            let results = evaluate_assertions(&scene);
            let passed = results.iter().all(|r| r.passed);
            print_json(&serde_json::json!({
                "ok": passed,
                "scene": scene.id,
                "assertions": results,
                "count": scene.assertions.len()
            }))?;
            if !passed {
                anyhow::bail!("Glyph assertions failed");
            }
            Ok(())
        }
        Command::State {
            input,
            name,
            output,
        } => {
            let scene = load_scene(&input)?;
            let sampled = scene_in_state(&scene, &name)?;
            fs::write(&output, serde_json::to_string_pretty(&sampled)? + "\n")?;
            print_json(
                &serde_json::json!({"ok":true,"scene":sampled.id,"state":name,"output":output}),
            )
        }
        Command::MotionSample {
            input,
            time,
            output,
        } => {
            let scene = load_scene(&input)?;
            let sampled = sample_motion(&scene, time)?;
            fs::write(&output, serde_json::to_string_pretty(&sampled)? + "\n")?;
            print_json(
                &serde_json::json!({"ok":true,"scene":sampled.id,"time":time,"output":output,"animations":sampled.animations.len()}),
            )
        }
        Command::SourceSet {
            input,
            node,
            property,
            value,
            output,
        } => {
            let source = fs::read_to_string(&input)?;
            let parsed_value = glyph::parse_value_literal(&value)?;
            let edited = set_source_property(&source, &node, &property, parsed_value)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"node":node,"property":property}),
            )
        }
        Command::Constraints { input, node } => {
            let scene = load_scene(&input)?;
            if let Some(node) = node {
                let constraints = scene
                    .constraints
                    .iter()
                    .filter(|constraint| {
                        constraint.lhs.node == node
                            || matches!(&constraint.rhs, ConstraintRhs::Property(reference) if reference.node == node)
                    })
                    .collect::<Vec<_>>();
                print_json(&constraints)
            } else {
                print_json(&scene.constraints)
            }
        }
        Command::ConstraintEdit {
            input,
            source_order,
            relation,
            strength,
            offset,
            output,
        } => {
            require_glyph_source(&input, "constraint-edit")?;
            if relation.is_none() && strength.is_none() && offset.is_none() {
                return Err(anyhow!(
                    "constraint-edit requires --relation, --strength, or --offset"
                ));
            }
            let mut edited = fs::read_to_string(&input)?;
            if let Some(relation) = relation {
                edited = set_source_constraint_relation(
                    &edited,
                    source_order,
                    parse_relation_arg(&relation)?,
                )?;
            }
            if let Some(strength) = strength {
                edited = set_source_constraint_strength(
                    &edited,
                    source_order,
                    parse_strength_arg(&strength)?,
                )?;
            }
            if let Some(offset) = offset {
                let (value, unit) = parse_constraint_offset_arg(&offset)?;
                edited = set_source_constraint_offset(&edited, source_order, value, unit)?;
            }
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"constraint":format!("constraint.{source_order}")}),
            )
        }
        Command::ConstraintRemove {
            input,
            source_order,
            output,
        } => {
            require_glyph_source(&input, "constraint-remove")?;
            let source = fs::read_to_string(&input)?;
            let edited = remove_source_constraint(&source, source_order)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"removed":format!("constraint.{source_order}")}),
            )
        }
        Command::ConstraintAdd {
            input,
            lhs,
            relation,
            rhs,
            offset,
            strength,
            output,
        } => {
            require_glyph_source(&input, "constraint-add")?;
            let lhs = parse_property_ref_arg(&lhs)?;
            let relation = parse_relation_arg(&relation)?;
            let rhs = parse_constraint_rhs_arg(&rhs)?;
            let (offset, offset_unit) = match offset {
                Some(raw) => parse_constraint_offset_arg(&raw)?,
                None => (0.0, None),
            };
            let strength = parse_strength_arg(&strength)?;
            let source = fs::read_to_string(&input)?;
            let edited = add_source_constraint(
                &source,
                &lhs,
                relation,
                &rhs,
                offset,
                offset_unit,
                strength,
            )?;
            fs::write(&output, edited)?;
            print_json(&serde_json::json!({"ok":true,"input":input,"output":output,"added":true}))
        }
        Command::GroupCreate {
            input,
            members,
            name,
            output,
        } => {
            require_glyph_source(&input, "group-create")?;
            let source = fs::read_to_string(&input)?;
            let (edited, group) = create_source_group(&source, name.as_deref(), &members)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"group":group,"members":members}),
            )
        }
        Command::GroupAddMember {
            input,
            group,
            member,
            output,
        } => {
            require_glyph_source(&input, "group-add-member")?;
            let source = fs::read_to_string(&input)?;
            let edited = add_source_group_member(&source, &group, &member)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"group":group,"member":member,"added":true}),
            )
        }
        Command::GroupRemoveMember {
            input,
            group,
            member,
            output,
        } => {
            require_glyph_source(&input, "group-remove-member")?;
            let source = fs::read_to_string(&input)?;
            let edited = remove_source_group_member(&source, &group, &member)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"group":group,"member":member,"removed":true}),
            )
        }
        Command::GroupRemove {
            input,
            group,
            output,
        } => {
            require_glyph_source(&input, "group-remove")?;
            let source = fs::read_to_string(&input)?;
            let edited = remove_source_group(&source, &group)?;
            fs::write(&output, edited)?;
            print_json(
                &serde_json::json!({"ok":true,"input":input,"output":output,"group":group,"removed":true}),
            )
        }
        Command::Hierarchy { input } => {
            require_glyph_source(&input, "hierarchy")?;
            let source = fs::read_to_string(&input)?;
            print_json(&serde_json::json!({"nodes":direct_source_nodes(&source)?}))
        }
        Command::NodeMove {
            input,
            node,
            anchor,
            placement,
            output,
        } => {
            require_glyph_source(&input, "node-move")?;
            let placement = placement
                .parse::<NodePlacement>()
                .map_err(anyhow::Error::msg)?;
            let source = fs::read_to_string(&input)?;
            let edited = move_source_node(&source, &node, &anchor, placement)?;
            fs::write(&output, edited)?;
            print_json(&serde_json::json!({
                "ok":true,"input":input,"output":output,"node":node,"anchor":anchor,
                "placement":match placement { NodePlacement::Before => "before", NodePlacement::After => "after" }
            }))
        }
        Command::DragPlan {
            input,
            node,
            dx,
            dy,
        } => {
            let scene = load_scene(&input)?;
            print_json(&plan_direct_manipulation(&scene, &node, dx, dy)?)
        }
        Command::SourceDrag {
            input,
            node,
            dx,
            dy,
            output,
        } => {
            if input.extension().and_then(|x| x.to_str()) != Some("glyph") {
                anyhow::bail!("source-drag requires a .glyph source input");
            }
            let scene = parse_file(&input)?;
            let plan = plan_direct_manipulation(&scene, &node, dx, dy)?;
            let source = fs::read_to_string(&input)?;
            let edited = apply_direct_manipulation_source(&source, &plan)?;
            fs::write(&output, edited)?;
            print_json(&serde_json::json!({"ok":true,"input":input,"output":output,"plan":plan}))
        }
        Command::ResizePlan {
            input,
            node,
            dw,
            dh,
        } => {
            let scene = load_scene(&input)?;
            print_json(&plan_direct_resize(&scene, &node, dw, dh)?)
        }
        Command::SourceResize {
            input,
            node,
            dw,
            dh,
            output,
        } => {
            if input.extension().and_then(|x| x.to_str()) != Some("glyph") {
                anyhow::bail!("source-resize requires a .glyph source input");
            }
            let scene = parse_file(&input)?;
            let plan = plan_direct_resize(&scene, &node, dw, dh)?;
            let source = fs::read_to_string(&input)?;
            let edited = apply_direct_resize_source(&source, &plan)?;
            fs::write(&output, edited)?;
            print_json(&serde_json::json!({"ok":true,"input":input,"output":output,"plan":plan}))
        }
        Command::ResizeHandlePlan {
            input,
            node,
            handle,
            dx,
            dy,
        } => {
            let scene = load_scene(&input)?;
            let handle = handle.parse::<ResizeHandle>().map_err(anyhow::Error::msg)?;
            print_json(&plan_direct_resize_from_handle(
                &scene, &node, handle, dx, dy,
            )?)
        }
        Command::SourceResizeHandle {
            input,
            node,
            handle,
            dx,
            dy,
            output,
        } => {
            if input.extension().and_then(|x| x.to_str()) != Some("glyph") {
                anyhow::bail!("source-resize-handle requires a .glyph source input");
            }
            let scene = parse_file(&input)?;
            let handle = handle.parse::<ResizeHandle>().map_err(anyhow::Error::msg)?;
            let plan = plan_direct_resize_from_handle(&scene, &node, handle, dx, dy)?;
            let source = fs::read_to_string(&input)?;
            let edited = apply_direct_handle_resize_source(&source, &plan)?;
            fs::write(&output, edited)?;
            print_json(&serde_json::json!({"ok":true,"input":input,"output":output,"plan":plan}))
        }
        Command::DisplayList { input, output } => {
            let scene = load_scene(&input)?;
            let display = build_display_list(&scene);
            if let Some(path) = output {
                fs::write(&path, serde_json::to_string_pretty(&display)? + "\n")?;
                print_json(
                    &serde_json::json!({"ok":true,"scene":scene.id,"output":path,"items":display.items.len(),"objects":display.object_map.len()}),
                )
            } else {
                print_json(&display)
            }
        }
        Command::ObjectMap { input } => {
            let scene = load_scene(&input)?;
            let display = build_display_list(&scene);
            print_json(&display.object_map)
        }
        Command::Conformance { input } => {
            let scene = load_scene(&input)?;
            print_json(&renderer_conformance_report(&scene))
        }
        Command::Accessibility { input } => {
            let scene = load_scene(&input)?;
            print_json(&accessibility_report(&scene))
        }
        Command::Diagnostics { input } => {
            require_glyph_source(&input, "diagnostics")?;
            let source = fs::read_to_string(&input)?;
            print_json(&glyph::diagnose_project_source(&input, &source))
        }
        Command::Complete {
            input,
            line,
            character,
        } => {
            require_glyph_source(&input, "complete")?;
            let source = fs::read_to_string(&input)?;
            print_json(&glyph::complete_source(
                &source,
                glyph::TextPosition { line, character },
            ))
        }
        Command::Lsp => glyph::lsp::run_stdio(),
        Command::Targets { input } => {
            let scene = load_scene(&input)?;
            print_json(&target_strategy_report(&scene))
        }
        Command::Diff { before, after } => {
            let before = load_scene(&before)?;
            let after = load_scene(&after)?;
            print_json(&diff_scenes(&before, &after))
        }
        Command::VisualDiff { before, after } => {
            let before = load_scene(&before)?;
            let after = load_scene(&after)?;
            print_json(&glyph::diff::visual_diff(&before, &after))
        }
    }
}
