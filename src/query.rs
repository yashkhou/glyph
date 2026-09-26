use crate::model::{Constraint, Node, Scene};
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

pub fn bounds(node: &Node) -> Option<[f64; 4]> {
    match node.kind {
        crate::model::NodeKind::Rect => Some([
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
        crate::model::NodeKind::Text => Some([n(node, "x")?, n(node, "y")?, 0.0, 0.0]),
        _ => None,
    }
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
