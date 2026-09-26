use crate::model::*;
use anyhow::{Context, Result, anyhow};
use serde_json::{Value as JsonValue, json};
use std::{fs, path::Path, process::Command};

fn n(node: &Node, key: &str, default: f64) -> f64 {
    node.derived
        .props
        .get(key)
        .copied()
        .or_else(|| node.authored.props.get(key).and_then(Value::number))
        .unwrap_or(default)
}
fn s(node: &Node, key: &str, default: &str) -> String {
    node.authored
        .props
        .get(key)
        .and_then(Value::text)
        .unwrap_or(default)
        .to_string()
}

pub fn commander_draw_document(scene: &Scene) -> JsonValue {
    let nodes: Vec<JsonValue> = scene.nodes.iter().filter_map(|node| {
        let style = json!({"fill": s(node,"fill","#000000")});
        let base = match node.kind {
            NodeKind::Rect => json!({"id":node.id,"type":"rect","name":node.name,"x":n(node,"x",0.0),"y":n(node,"y",0.0),"width":n(node,"width",0.0),"height":n(node,"height",0.0),"rx":n(node,"rx",0.0),"style":style}),
            NodeKind::Ellipse => json!({"id":node.id,"type":"ellipse","name":node.name,"cx":n(node,"cx",0.0),"cy":n(node,"cy",0.0),"rx":n(node,"rx",0.0),"ry":n(node,"ry",0.0),"style":style}),
            NodeKind::Path => json!({"id":node.id,"type":"path","name":node.name,"d":s(node,"d",""),"style":style}),
            NodeKind::Text => json!({"id":node.id,"type":"text","name":node.name,"text":s(node,"content",""),"x":n(node,"x",0.0),"y":n(node,"y",0.0),"fontSize":n(node,"size",16.0),"fontFamily":s(node,"font","system-ui"),"fontWeight":s(node,"weight","400"),"textAnchor":match s(node,"align","start").as_str(){"center"=>"middle","right"=>"end",_=>"start"},"style":style}),
            NodeKind::Group => return None,
        };
        Some(base)
    }).collect();
    json!({
        "schemaVersion":1,
        "id":format!("draw-{}",scene.id),
        "name":scene.name,
        "width":scene.canvas.width,
        "height":scene.canvas.height,
        "background":scene.canvas.background,
        "createdAt":"1970-01-01T00:00:00.000Z",
        "updatedAt":"1970-01-01T00:00:00.000Z",
        "nodes":nodes,
        "meta":{"engine":"Glyph -> Commander Draw bridge","units":"px","glyphSceneId":scene.id,"glyphRevision":scene.revision}
    })
}

pub fn write_commander_draw(scene: &Scene, path: impl AsRef<Path>) -> Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(&commander_draw_document(scene))? + "\n",
    )?;
    Ok(())
}

pub fn render_png_with_commander_draw(
    scene: &Scene,
    output: impl AsRef<Path>,
    commander_root: impl AsRef<Path>,
) -> Result<()> {
    let tmp = std::env::temp_dir().join(format!(
        "glyph-{}-{}.draw.json",
        std::process::id(),
        scene.revision
    ));
    write_commander_draw(scene, &tmp)?;
    let cli = commander_root.as_ref().join("src/cli.mjs");
    if !cli.exists() {
        return Err(anyhow!("CommanderDraw CLI not found at {}", cli.display()));
    }
    let result = Command::new("node")
        .arg(cli)
        .arg("render")
        .arg(&tmp)
        .arg(output.as_ref())
        .arg("1")
        .output()
        .context("failed to invoke CommanderDraw")?;
    let _ = fs::remove_file(&tmp);
    if !result.status.success() {
        return Err(anyhow!(
            "CommanderDraw render failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}
