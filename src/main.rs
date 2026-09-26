use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use glyph::{
    PatchOperation, Scene, apply_transaction, diff::diff_scenes, parse_scene, query, render_svg,
    solve_scene,
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
    Diff {
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
    let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut scene = parse_scene(&source, &path.display().to_string())?;
    let _ = solve_scene(&mut scene);
    Ok(scene)
}

fn load_gsir(path: &Path) -> Result<Scene> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capabilities => print_json(&serde_json::json!({
            "name":"Glyph Core", "version":env!("CARGO_PKG_VERSION"),
            "canonical":"GSIR", "sourceExtension":".glyph",
            "nodeKinds":["rect","ellipse","path","text","group"],
            "constraints":["==",">=","<=","required","strong","medium","weak"],
            "agent":["query-by-role","inspect","why","revision-safe-apply","semantic-diff"],
            "outputs":["gsir-json","svg","png-via-CommanderDraw","commander-draw-json"],
            "guarantees":["stable semantic IDs","authored/derived separation","transaction revision checks","deterministic SVG"],
            "status":{"vision":"postponed","motion":"postponed","studio":"postponed","solver":"MVP deterministic resolver; Cassowary-class solver planned"}
        })),
        Command::Check { input } => {
            let source = fs::read_to_string(&input)?;
            let mut scene = parse_scene(&source, &input.display().to_string())?;
            let report = solve_scene(&mut scene);
            print_json(
                &serde_json::json!({"ok":report.required_conflicts.is_empty() && report.unresolved.is_empty(),"scene":scene.id,"revision":scene.revision,"nodes":scene.nodes.len(),"constraints":scene.constraints.len(),"solve":report}),
            )
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
        Command::Diff { before, after } => {
            let before = load_scene(&before)?;
            let after = load_scene(&after)?;
            print_json(&diff_scenes(&before, &after))
        }
    }
}
