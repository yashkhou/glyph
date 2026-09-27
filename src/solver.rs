use crate::{
    model::*,
    text_layout::{apply_text_metrics, text_left},
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveReport {
    pub applied: Vec<String>,
    pub relaxed: Vec<String>,
    pub unresolved: Vec<String>,
    pub required_conflicts: Vec<String>,
}

fn authored_number(node: &Node, key: &str) -> Option<f64> {
    node.authored.props.get(key).and_then(Value::number)
}

fn derived_number(node: &Node, key: &str) -> Option<f64> {
    node.derived
        .props
        .get(key)
        .copied()
        .or_else(|| authored_number(node, key))
}

fn node_prop(node: &Node, prop: &str) -> Option<f64> {
    match node.kind {
        NodeKind::Rect => {
            let x = derived_number(node, "x")?;
            let y = derived_number(node, "y")?;
            let w = derived_number(node, "width")?;
            let h = derived_number(node, "height")?;
            match prop {
                "x" | "left" => Some(x),
                "y" | "top" => Some(y),
                "width" => Some(w),
                "height" => Some(h),
                "right" => Some(x + w),
                "bottom" => Some(y + h),
                "center_x" => Some(x + w / 2.0),
                "center_y" => Some(y + h / 2.0),
                _ => derived_number(node, prop),
            }
        }
        NodeKind::Ellipse => {
            let cx = derived_number(node, "cx")?;
            let cy = derived_number(node, "cy")?;
            let rx = derived_number(node, "rx")?;
            let ry = derived_number(node, "ry")?;
            match prop {
                "cx" | "center_x" => Some(cx),
                "cy" | "center_y" => Some(cy),
                "rx" => Some(rx),
                "ry" => Some(ry),
                "width" => Some(rx * 2.0),
                "height" => Some(ry * 2.0),
                "left" => Some(cx - rx),
                "right" => Some(cx + rx),
                "top" => Some(cy - ry),
                "bottom" => Some(cy + ry),
                _ => derived_number(node, prop),
            }
        }
        NodeKind::Text => {
            let x = derived_number(node, "x")?;
            let y = derived_number(node, "y")?;
            let w = derived_number(node, "width").unwrap_or(0.0);
            let h = derived_number(node, "height").unwrap_or(0.0);
            let ascent = derived_number(node, "ascent").unwrap_or(0.0);
            let left = text_left(node);
            let top = y - ascent;
            match prop {
                "x" => Some(x),
                "y" => Some(y),
                "width" => Some(w),
                "height" => Some(h),
                "left" => Some(left),
                "right" => Some(left + w),
                "top" => Some(top),
                "bottom" => Some(top + h),
                "center_x" => Some(left + w / 2.0),
                "center_y" => Some(top + h / 2.0),
                _ => derived_number(node, prop),
            }
        }
        _ => derived_number(node, prop),
    }
}

fn set_node_prop(node: &mut Node, prop: &str, value: f64) -> Result<()> {
    let rect_w = node_prop(node, "width").unwrap_or(0.0);
    let rect_h = node_prop(node, "height").unwrap_or(0.0);
    let ellipse_rx = node_prop(node, "rx").unwrap_or(0.0);
    let ellipse_ry = node_prop(node, "ry").unwrap_or(0.0);
    let text_ascent = derived_number(node, "ascent").unwrap_or(0.0);
    let text_align = node
        .authored
        .props
        .get("align")
        .and_then(Value::text)
        .unwrap_or("start")
        .to_string();
    let mut set = |k: &str, v: f64| {
        node.derived.props.insert(k.to_string(), v);
    };
    match node.kind {
        NodeKind::Rect => {
            let w = rect_w;
            let h = rect_h;
            match prop {
                "x" | "left" => set("x", value),
                "y" | "top" => set("y", value),
                "width" => set("width", value),
                "height" => set("height", value),
                "right" => set("x", value - w),
                "bottom" => set("y", value - h),
                "center_x" => set("x", value - w / 2.0),
                "center_y" => set("y", value - h / 2.0),
                other => set(other, value),
            }
        }
        NodeKind::Ellipse => {
            let rx = ellipse_rx;
            let ry = ellipse_ry;
            match prop {
                "cx" | "center_x" => set("cx", value),
                "cy" | "center_y" => set("cy", value),
                "rx" => set("rx", value),
                "ry" => set("ry", value),
                "width" => set("rx", value / 2.0),
                "height" => set("ry", value / 2.0),
                "left" => set("cx", value + rx),
                "right" => set("cx", value - rx),
                "top" => set("cy", value + ry),
                "bottom" => set("cy", value - ry),
                other => set(other, value),
            }
        }
        NodeKind::Text => {
            let w = rect_w;
            let h = rect_h;
            let ascent = text_ascent;
            let align = text_align.as_str();
            match prop {
                "x" => set("x", value),
                "y" => set("y", value),
                "left" => set(
                    "x",
                    match align {
                        "center" => value + w / 2.0,
                        "right" => value + w,
                        _ => value,
                    },
                ),
                "right" => set(
                    "x",
                    match align {
                        "center" => value - w / 2.0,
                        "right" => value,
                        _ => value - w,
                    },
                ),
                "center_x" => set(
                    "x",
                    match align {
                        "center" => value,
                        "right" => value + w / 2.0,
                        _ => value - w / 2.0,
                    },
                ),
                "top" => set("y", value + ascent),
                "bottom" => set("y", value - h + ascent),
                "center_y" => set("y", value - h / 2.0 + ascent),
                other => set(other, value),
            }
        }
        _ => set(prop, value),
    }
    Ok(())
}

fn canvas_prop(canvas: &Canvas, prop: &str) -> Option<f64> {
    match prop {
        "x" | "left" | "y" | "top" => Some(0.0),
        "width" | "right" => Some(canvas.width),
        "height" | "bottom" => Some(canvas.height),
        "center_x" => Some(canvas.width / 2.0),
        "center_y" => Some(canvas.height / 2.0),
        _ => None,
    }
}

pub fn property_value(scene: &Scene, r: &PropertyRef) -> Option<f64> {
    if r.node == "canvas" {
        return canvas_prop(&scene.canvas, &r.property);
    }
    scene
        .nodes
        .iter()
        .find(|n| n.id == r.node || n.name == r.node)
        .and_then(|n| node_prop(n, &r.property))
}

pub fn set_property_value(scene: &mut Scene, r: &PropertyRef, value: f64) -> Result<()> {
    if r.node == "canvas" {
        return Err(anyhow!("canvas properties are immutable in constraints"));
    }
    let node = scene
        .nodes
        .iter_mut()
        .find(|n| n.id == r.node || n.name == r.node)
        .ok_or_else(|| anyhow!("unknown node {}", r.node))?;
    set_node_prop(node, &r.property, value)
}

pub fn solve_scene_legacy(scene: &mut Scene) -> SolveReport {
    for node in &mut scene.nodes {
        node.derived.props.clear();
        node.derived.relaxed_constraints.clear();
        for (k, v) in &node.authored.props {
            if let Some(n) = v.number() {
                node.derived.props.insert(k.clone(), n);
            }
        }
        apply_text_metrics(node);
    }

    let mut constraints = scene.constraints.clone();
    constraints.sort_by_key(|c| (std::cmp::Reverse(c.strength.rank()), c.source_order));
    let mut writers: HashMap<String, (u8, String, f64)> = HashMap::new();
    let mut report = SolveReport {
        applied: vec![],
        relaxed: vec![],
        unresolved: vec![],
        required_conflicts: vec![],
    };

    let max_passes = constraints.len().saturating_add(1).max(2);
    for _pass in 0..max_passes {
        let mut changed = false;
        for c in &constraints {
            let rhs = match &c.rhs {
                ConstraintRhs::Number(n) => Some(*n),
                ConstraintRhs::Quantity(q) => Some(q.canonical_value()),
                ConstraintRhs::Property(r) => property_value(scene, r),
            };
            let Some(rhs) = rhs else {
                if !report.unresolved.contains(&c.id) {
                    report.unresolved.push(c.id.clone());
                }
                continue;
            };
            let target = rhs + c.offset;
            let current = property_value(scene, &c.lhs);
            let desired = match (c.relation, current) {
                (Relation::Eq, _) => target,
                (Relation::Ge, Some(v)) if v >= target - 0.0001 => continue,
                (Relation::Ge, _) => target,
                (Relation::Le, Some(v)) if v <= target + 0.0001 => continue,
                (Relation::Le, _) => target,
            };
            let key = format!("{}.{}", c.lhs.node, c.lhs.property);
            if let Some((rank, writer, prior)) = writers.get(&key).cloned() {
                if writer != c.id && rank >= c.strength.rank() {
                    if c.strength == Strength::Required
                        && rank == Strength::Required.rank()
                        && (prior - desired).abs() > 0.001
                    {
                        let msg = format!(
                            "{} conflicts with {} on {} ({} vs {})",
                            c.id, writer, key, desired, prior
                        );
                        if !report.required_conflicts.contains(&msg) {
                            report.required_conflicts.push(msg);
                        }
                    } else if !report.relaxed.contains(&c.id) {
                        report.relaxed.push(c.id.clone());
                    }
                    continue;
                }
            }
            if set_property_value(scene, &c.lhs, desired).is_ok() {
                let before = current.unwrap_or(f64::NAN);
                if before.is_nan() || (before - desired).abs() > 0.0001 {
                    changed = true;
                }
                writers.insert(key, (c.strength.rank(), c.id.clone(), desired));
                if !report.applied.contains(&c.id) {
                    report.applied.push(c.id.clone());
                }
            } else if !report.unresolved.contains(&c.id) {
                report.unresolved.push(c.id.clone());
            }
        }
        if !changed {
            break;
        }
    }

    for c in &constraints {
        if c.strength != Strength::Required {
            continue;
        }
        let rhs = match &c.rhs {
            ConstraintRhs::Number(n) => Some(*n),
            ConstraintRhs::Quantity(q) => Some(q.canonical_value()),
            ConstraintRhs::Property(r) => property_value(scene, r),
        };
        let lhs = property_value(scene, &c.lhs);
        let Some((lhs, rhs)) = lhs.zip(rhs) else {
            if !report.unresolved.contains(&c.id) {
                report.unresolved.push(c.id.clone());
            }
            continue;
        };
        let target = rhs + c.offset;
        let satisfied = match c.relation {
            Relation::Eq => (lhs - target).abs() <= 0.0001,
            Relation::Ge => lhs >= target - 0.0001,
            Relation::Le => lhs <= target + 0.0001,
        };
        if !satisfied && !report.required_conflicts.iter().any(|m| m.contains(&c.id)) {
            report.required_conflicts.push(format!(
                "{} remains unsatisfied after solving (lhs {} vs target {})",
                c.id, lhs, target
            ));
        }
    }

    for id in &report.relaxed {
        if let Some(c) = scene.constraints.iter().find(|c| &c.id == id) {
            if let Some(n) = scene
                .nodes
                .iter_mut()
                .find(|n| n.id == c.lhs.node || n.name == c.lhs.node)
            {
                n.derived.relaxed_constraints.push(id.clone());
            }
        }
    }
    report
}

pub fn solve_scene(scene: &mut Scene) -> SolveReport {
    crate::linear_solver::solve_scene_linear(scene)
}
