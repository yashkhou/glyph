use crate::model::*;
use anyhow::Result;
use std::fs;
use std::path::Path;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(node: &Node, key: &str, default: f64) -> f64 {
    node.derived
        .props
        .get(key)
        .copied()
        .or_else(|| node.authored.props.get(key).and_then(Value::number))
        .unwrap_or(default)
}

fn text(node: &Node, key: &str, default: &str) -> String {
    node.authored
        .props
        .get(key)
        .and_then(Value::text)
        .unwrap_or(default)
        .to_string()
}

fn common(node: &Node) -> String {
    let roles = esc(&node.roles.join(" "));
    format!("id=\"{}\" data-glyph-role=\"{}\"", esc(&node.id), roles)
}

pub fn render_svg(scene: &Scene) -> String {
    let mut body = String::new();
    if let Some(bg) = &scene.canvas.background {
        body.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            scene.canvas.width,
            scene.canvas.height,
            esc(bg)
        ));
    }
    for node in &scene.nodes {
        let fill = esc(&text(node, "fill", "#000000"));
        match node.kind {
            NodeKind::Rect => body.push_str(&format!(
                "<rect {} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{}\"/>\n",
                common(node),
                num(node, "x", 0.0),
                num(node, "y", 0.0),
                num(node, "width", 0.0),
                num(node, "height", 0.0),
                num(node, "rx", 0.0),
                fill
            )),
            NodeKind::Ellipse => body.push_str(&format!(
                "<ellipse {} cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\"/>\n",
                common(node),
                num(node, "cx", 0.0),
                num(node, "cy", 0.0),
                num(node, "rx", 0.0),
                num(node, "ry", 0.0),
                fill
            )),
            NodeKind::Path => body.push_str(&format!(
                "<path {} d=\"{}\" fill=\"{}\"/>\n",
                common(node),
                esc(&text(node, "d", "")),
                fill
            )),
            NodeKind::Text => {
                let anchor = match text(node, "align", "start").as_str() {
                    "center" => "middle",
                    "right" => "end",
                    _ => "start",
                };
                body.push_str(&format!(
                    "<text {} x=\"{}\" y=\"{}\" font-size=\"{}\" font-family=\"{}\" font-weight=\"{}\" text-anchor=\"{}\" fill=\"{}\">{}</text>\n",
                    common(node), num(node,"x",0.0), num(node,"y",0.0), num(node,"size",16.0), esc(&text(node,"font","system-ui")), esc(&text(node,"weight","400")), anchor, fill, esc(&text(node,"content",""))));
            }
            NodeKind::Group => body.push_str(&format!("<g {}></g>\n", common(node))),
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n{}</svg>\n",
        scene.canvas.width, scene.canvas.height, scene.canvas.width, scene.canvas.height, body
    )
}

pub fn write_svg(scene: &Scene, path: impl AsRef<Path>) -> Result<()> {
    fs::write(path, render_svg(scene))?;
    Ok(())
}
