use crate::{
    model::{Constraint, Node, Scene, Value},
    text_layout::text_left,
};
use kurbo::{BezPath, Shape};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub roles: Vec<String>,
    pub bounds: Option<[f64; 4]>,
}

fn n(node: &Node, key: &str) -> Option<f64> {
    node.derived
        .props
        .get(key)
        .copied()
        .or_else(|| node.authored.props.get(key).and_then(|v| v.number()))
}

fn base_bounds(node: &Node) -> Option<[f64; 4]> {
    match node.kind {
        crate::model::NodeKind::Rect | crate::model::NodeKind::Image => Some([
            n(node, "x")?,
            n(node, "y")?,
            n(node, "width")?,
            n(node, "height")?,
        ]),
        crate::model::NodeKind::Ellipse => {
            let cx = n(node, "cx")?;
            let cy = n(node, "cy")?;
            let rx = n(node, "rx")?;
            let ry = n(node, "ry")?;
            Some([cx - rx, cy - ry, rx * 2.0, ry * 2.0])
        }
        crate::model::NodeKind::Text => {
            let y = n(node, "y")?;
            let h = n(node, "height")?;
            let ascent = n(node, "ascent").unwrap_or(0.0);
            Some([text_left(node), y - ascent, n(node, "width")?, h])
        }
        crate::model::NodeKind::Path => {
            let d = node.authored.props.get("d").and_then(Value::text)?;
            let path = BezPath::from_svg(d).ok()?;
            if path.elements().is_empty() {
                return None;
            }
            let rect = path.bounding_box();
            Some([rect.x0, rect.y0, rect.width(), rect.height()])
        }
        crate::model::NodeKind::Group => None,
    }
}

fn transformed_bounds(node: &Node, bounds: [f64; 4]) -> [f64; 4] {
    let scale = n(node, "scale").unwrap_or(1.0);
    let sx = n(node, "scale_x").unwrap_or(scale);
    let sy = n(node, "scale_y").unwrap_or(scale);
    let rotation = n(node, "rotate")
        .or_else(|| n(node, "rotation"))
        .unwrap_or(0.0)
        .to_radians();
    let tx = n(node, "translate_x").unwrap_or(0.0);
    let ty = n(node, "translate_y").unwrap_or(0.0);
    let ox = n(node, "origin_x").unwrap_or(0.0);
    let oy = n(node, "origin_y").unwrap_or(0.0);
    let (sin, cos) = rotation.sin_cos();
    let [x, y, width, height] = bounds;
    let corners = [
        (x, y),
        (x + width, y),
        (x, y + height),
        (x + width, y + height),
    ];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (px, py) in corners {
        let lx = (px - ox) * sx;
        let ly = (py - oy) * sy;
        let px = tx + ox + cos * lx - sin * ly;
        let py = ty + oy + sin * lx + cos * ly;
        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
    }
    [min_x, min_y, max_x - min_x, max_y - min_y]
}

pub fn bounds(node: &Node) -> Option<[f64; 4]> {
    Some(transformed_bounds(node, base_bounds(node)?))
}

pub fn by_role(scene: &Scene, role: &str) -> Vec<NodeSummary> {
    scene
        .nodes
        .iter()
        .filter(|n| n.roles.iter().any(|r| r == role))
        .map(|n| NodeSummary {
            id: n.id.clone(),
            name: n.name.clone(),
            kind: format!("{:?}", n.kind).to_lowercase(),
            roles: n.roles.clone(),
            bounds: bounds(n),
        })
        .collect()
}

pub fn constraints_for<'a>(scene: &'a Scene, node: &str) -> Vec<&'a Constraint> {
    scene
        .constraints
        .iter()
        .filter(|c| {
            c.lhs.node == node
                || match &c.rhs {
                    crate::model::ConstraintRhs::Property(r) => r.node == node,
                    _ => false,
                }
        })
        .collect()
}
