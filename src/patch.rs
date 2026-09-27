use crate::{diff::diff_scenes, model::*, solver::solve_scene};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOperation {
    Set {
        node: String,
        path: String,
        value: Value,
    },
    AddRole {
        node: String,
        role: String,
    },
    RemoveRole {
        node: String,
        role: String,
    },
    SetCanvas {
        width: Option<f64>,
        height: Option<f64>,
        background: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub from_revision: u64,
    pub to_revision: u64,
    pub changed_authored_fields: usize,
    pub unrelated_authored_nodes_changed: usize,
    pub required_constraints_violated: usize,
    pub solve_report: crate::solver::SolveReport,
}

pub fn apply_transaction(
    scene: &mut Scene,
    base_revision: u64,
    operations: &[PatchOperation],
) -> Result<TransactionResult> {
    if scene.revision != base_revision {
        return Err(anyhow!(
            "revision conflict: expected {}, current {}",
            base_revision,
            scene.revision
        ));
    }
    let before = scene.clone();
    let explicitly_touched: std::collections::BTreeSet<String> = operations
        .iter()
        .map(|op| match op {
            PatchOperation::Set { node, .. }
            | PatchOperation::AddRole { node, .. }
            | PatchOperation::RemoveRole { node, .. } => node.clone(),
            PatchOperation::SetCanvas { .. } => "@canvas".into(),
        })
        .collect();

    for op in operations {
        match op {
            PatchOperation::Set { node, path, value } => {
                let target = scene
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == *node || n.name == *node)
                    .ok_or_else(|| anyhow!("unknown node {node}"))?;
                let key = path.strip_prefix("props.").unwrap_or(path);
                validate_value_for_property(key, value).map_err(|message| anyhow!(message))?;
                target.authored.props.insert(key.to_string(), value.clone());
            }
            PatchOperation::AddRole { node, role } => {
                let target = scene
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == *node || n.name == *node)
                    .ok_or_else(|| anyhow!("unknown node {node}"))?;
                if !target.roles.contains(role) {
                    target.roles.push(role.clone());
                }
            }
            PatchOperation::RemoveRole { node, role } => {
                let target = scene
                    .nodes
                    .iter_mut()
                    .find(|n| n.id == *node || n.name == *node)
                    .ok_or_else(|| anyhow!("unknown node {node}"))?;
                target.roles.retain(|r| r != role);
            }
            PatchOperation::SetCanvas {
                width,
                height,
                background,
            } => {
                if let Some(v) = width {
                    scene.canvas.width = *v;
                }
                if let Some(v) = height {
                    scene.canvas.height = *v;
                }
                if let Some(v) = background {
                    scene.canvas.background = Some(v.clone());
                }
            }
        }
    }

    scene.revision += 1;
    let solve_report = solve_scene(scene);
    let diff = diff_scenes(&before, scene);
    let changed_nodes: std::collections::BTreeSet<String> = diff
        .iter()
        .filter(|d| d.category == "authored" || d.category == "semantic")
        .map(|d| d.node.clone())
        .collect();
    let unrelated = changed_nodes.difference(&explicitly_touched).count();

    Ok(TransactionResult {
        from_revision: before.revision,
        to_revision: scene.revision,
        changed_authored_fields: diff
            .iter()
            .filter(|d| d.category == "authored" || d.category == "semantic")
            .count(),
        unrelated_authored_nodes_changed: unrelated,
        required_constraints_violated: solve_report.required_conflicts.len(),
        solve_report,
    })
}
