use crate::{
    model::{Constraint, Node, NodeKind, PropertyRef, Quantity, Relation, Scene, Unit, Value},
    query,
    solver::{property_value, solve_scene},
    source_edit::{
        set_source_constraint_offset, set_source_instance_parameter, set_source_property,
    },
};
use anyhow::{Result, anyhow};
use kurbo::{BezPath, Shape};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManipulationAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManipulationEdit {
    pub property: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintOffsetEdit {
    pub constraint_id: String,
    pub source_order: usize,
    pub axis: ManipulationAxis,
    pub previous_offset: f64,
    pub next_offset: f64,
    pub unit: Option<Unit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentParameterEdit {
    pub instance_id: String,
    pub parameter: String,
    pub target_property: String,
    pub axis: ManipulationAxis,
    pub previous_value: Value,
    pub next_value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManipulationBlock {
    pub axis: ManipulationAxis,
    pub constraint_ids: Vec<String>,
    pub properties: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectManipulationPlan {
    pub node_id: String,
    pub dx: f64,
    pub dy: f64,
    pub allowed: bool,
    pub edits: Vec<ManipulationEdit>,
    pub constraint_edits: Vec<ConstraintOffsetEdit>,
    pub instance_parameter_edits: Vec<ComponentParameterEdit>,
    pub blocked: Vec<ManipulationBlock>,
    pub note: String,
}

fn axis_property(node: &Node, axis: ManipulationAxis) -> Option<&'static str> {
    match (node.kind.clone(), axis) {
        (NodeKind::Rect | NodeKind::Image | NodeKind::Text, ManipulationAxis::Horizontal) => {
            Some("x")
        }
        (NodeKind::Rect | NodeKind::Image | NodeKind::Text, ManipulationAxis::Vertical) => {
            Some("y")
        }
        (NodeKind::Ellipse, ManipulationAxis::Horizontal) => Some("cx"),
        (NodeKind::Ellipse, ManipulationAxis::Vertical) => Some("cy"),
        (NodeKind::Path, ManipulationAxis::Horizontal) => Some("translate_x"),
        (NodeKind::Path, ManipulationAxis::Vertical) => Some("translate_y"),
        (NodeKind::Group, _) => None,
    }
}

fn positional_property(property: &str, axis: ManipulationAxis) -> bool {
    match axis {
        ManipulationAxis::Horizontal => matches!(
            property,
            "x" | "left" | "right" | "center_x" | "cx" | "translate_x"
        ),
        ManipulationAxis::Vertical => matches!(
            property,
            "y" | "top" | "bottom" | "center_y" | "cy" | "baseline" | "translate_y"
        ),
    }
}

fn blocking_constraints<'a>(
    scene: &'a Scene,
    node_id: &str,
    axis: ManipulationAxis,
) -> Vec<&'a Constraint> {
    scene
        .constraints
        .iter()
        .filter(|constraint| {
            constraint.lhs.node == node_id && positional_property(&constraint.lhs.property, axis)
        })
        .collect()
}

fn moved_value(node: &Node, property: &str, delta: f64) -> Result<Value> {
    let current = node
        .derived
        .props
        .get(property)
        .copied()
        .or_else(|| node.authored.props.get(property).and_then(Value::number))
        .unwrap_or(0.0);
    let next = current + delta;
    match node.authored.props.get(property) {
        Some(Value::Quantity(quantity)) if quantity.unit == Unit::Px => {
            Ok(Value::Quantity(Quantity {
                value: next,
                unit: Unit::Px,
            }))
        }
        Some(Value::Quantity(quantity)) => Err(anyhow!(
            "direct manipulation cannot preserve non-pixel unit {:?} for {}.{}",
            quantity.unit,
            node.id,
            property
        )),
        Some(Value::Text(_)) => Err(anyhow!(
            "direct manipulation requires numeric property {}.{}",
            node.id,
            property
        )),
        _ => Ok(Value::Number(next)),
    }
}

fn component_local_value(node: &Node, property: &str) -> Result<Value> {
    let origin = node
        .component_origin
        .as_ref()
        .ok_or_else(|| anyhow!("node {} has no component origin metadata", node.id))?;
    if !origin.scale.is_finite() || origin.scale <= 0.0 {
        return Err(anyhow!(
            "component instance {} has unsupported non-positive scale {}",
            origin.instance_id,
            origin.scale
        ));
    }
    let authored = node
        .authored
        .props
        .get(property)
        .ok_or_else(|| anyhow!("component child {} has no authored {property}", node.id))?;
    let transformed = authored
        .number()
        .ok_or_else(|| anyhow!("component child {}.{} is not numeric", node.id, property))?;
    let local = match property {
        "x" | "cx" => (transformed - origin.offset_x) / origin.scale,
        "y" | "cy" => (transformed - origin.offset_y) / origin.scale,
        _ => transformed / origin.scale,
    };
    Ok(match authored {
        Value::Quantity(quantity) => Value::Quantity(Quantity {
            value: local,
            unit: quantity.unit,
        }),
        Value::Number(_) => Value::Number(local),
        Value::Text(_) => unreachable!("numeric value checked above"),
    })
}

fn component_parameter_edit(
    node: &Node,
    axis: ManipulationAxis,
    property: &str,
    delta: f64,
) -> Result<(Option<ComponentParameterEdit>, Option<ManipulationBlock>)> {
    let Some(origin) = node.component_origin.as_ref() else {
        return Ok((
            None,
            Some(ManipulationBlock {
                axis,
                constraint_ids: vec![],
                properties: vec![property.into()],
                reason: "component child is missing inverse-authoring provenance".into(),
            }),
        ));
    };
    if origin.overridden_properties.iter().any(|p| p == property) {
        return Ok((
            None,
            Some(ManipulationBlock {
                axis,
                constraint_ids: vec![],
                properties: vec![property.into()],
                reason: format!(
                    "{}.{} is controlled by an instance override; edit that override explicitly",
                    origin.local_id, property
                ),
            }),
        ));
    }
    let Some(bindings) = origin.parameter_bindings.get(property) else {
        return Ok((
            None,
            Some(ManipulationBlock {
                axis,
                constraint_ids: vec![],
                properties: vec![property.into()],
                reason: format!(
                    "{}.{} has no component parameter binding to invert",
                    origin.local_id, property
                ),
            }),
        ));
    };
    if bindings.len() != 1 {
        return Ok((
            None,
            Some(ManipulationBlock {
                axis,
                constraint_ids: vec![],
                properties: vec![property.into()],
                reason: format!(
                    "{}.{} is controlled by multiple component parameters ({})",
                    origin.local_id,
                    property,
                    bindings.join(", ")
                ),
            }),
        ));
    }
    if !origin.scale.is_finite() || origin.scale <= 0.0 {
        return Ok((
            None,
            Some(ManipulationBlock {
                axis,
                constraint_ids: vec![],
                properties: vec![property.into()],
                reason: format!(
                    "component instance {} has unsupported scale {} for inverse manipulation",
                    origin.instance_id, origin.scale
                ),
            }),
        ));
    }
    let previous_value = component_local_value(node, property)?;
    let next_number = previous_value
        .number()
        .ok_or_else(|| anyhow!("component parameter {} is not numeric", bindings[0]))?
        + delta / origin.scale;
    let next_value = match previous_value {
        Value::Quantity(quantity) => Value::Quantity(Quantity {
            value: next_number,
            unit: quantity.unit,
        }),
        Value::Number(_) => Value::Number(next_number),
        Value::Text(_) => unreachable!("numeric component parameter checked above"),
    };
    Ok((
        Some(ComponentParameterEdit {
            instance_id: origin.instance_id.clone(),
            parameter: bindings[0].clone(),
            target_property: property.into(),
            axis,
            previous_value,
            next_value,
        }),
        None,
    ))
}

fn axis_ref(node: &Node, axis: ManipulationAxis) -> Option<PropertyRef> {
    axis_property(node, axis).map(|property| PropertyRef {
        node: node.id.clone(),
        property: property.into(),
    })
}

fn candidate_axis_motion_is_exact(
    scene: &Scene,
    node: &Node,
    axis: ManipulationAxis,
    delta: f64,
    direct_edit: Option<&ManipulationEdit>,
    constraint_edits: &[ConstraintOffsetEdit],
    controlling_ids: &[String],
) -> Result<(), String> {
    let Some(reference) = axis_ref(node, axis) else {
        return Err("node has no measurable authored position for this axis".into());
    };
    let mut baseline = scene.clone();
    let baseline_report = solve_scene(&mut baseline);
    if !baseline_report.required_conflicts.is_empty() || !baseline_report.unresolved.is_empty() {
        return Err("baseline scene is not fully solvable".into());
    }
    let before = property_value(&baseline, &reference)
        .ok_or_else(|| format!("cannot measure {}.{}", reference.node, reference.property))?;

    let mut candidate = baseline.clone();
    if let Some(edit) = direct_edit {
        let target = candidate
            .nodes
            .iter_mut()
            .find(|candidate_node| candidate_node.id == node.id)
            .ok_or_else(|| format!("candidate node {} disappeared", node.id))?;
        target
            .authored
            .props
            .insert(edit.property.clone(), edit.value.clone());
    }
    for edit in constraint_edits {
        let constraint = candidate
            .constraints
            .iter_mut()
            .find(|constraint| constraint.source_order == edit.source_order)
            .ok_or_else(|| format!("{} disappeared from candidate scene", edit.constraint_id))?;
        constraint.offset = edit.next_offset;
        constraint.offset_unit = edit.unit;
    }

    let report = solve_scene(&mut candidate);
    if !report.required_conflicts.is_empty() || !report.unresolved.is_empty() {
        return Err(
            "candidate edit introduces an unresolved or required constraint conflict".into(),
        );
    }
    if report
        .relaxed
        .iter()
        .any(|id| controlling_ids.iter().any(|control| control == id))
    {
        return Err("candidate edit would relax a controlling positional constraint".into());
    }
    if report.relaxed.iter().any(|id| {
        !baseline_report
            .relaxed
            .iter()
            .any(|baseline_id| baseline_id == id)
    }) {
        return Err("candidate edit would newly relax another authored constraint".into());
    }
    let after = property_value(&candidate, &reference).ok_or_else(|| {
        format!(
            "cannot re-measure {}.{}",
            reference.node, reference.property
        )
    })?;
    let actual = after - before;
    if (actual - delta).abs() > 0.0001 {
        return Err(format!(
            "candidate edit resolves to delta {actual:.4}, not requested delta {delta:.4}"
        ));
    }
    Ok(())
}

fn axis_constraint_resolution(
    scene: &Scene,
    node: &Node,
    axis: ManipulationAxis,
    property: &str,
    delta: f64,
) -> Result<(Vec<ConstraintOffsetEdit>, Option<ManipulationBlock>)> {
    let constraints = blocking_constraints(scene, &node.id, axis);
    if constraints.is_empty() {
        return Ok((vec![], None));
    }
    let ids = constraints
        .iter()
        .map(|constraint| constraint.id.clone())
        .collect::<Vec<_>>();
    let properties = constraints
        .iter()
        .map(|constraint| constraint.lhs.property.clone())
        .collect::<Vec<_>>();
    let equalities = constraints
        .iter()
        .copied()
        .filter(|constraint| constraint.relation == Relation::Eq)
        .collect::<Vec<_>>();

    if equalities
        .iter()
        .any(|constraint| !matches!(constraint.offset_unit, None | Some(Unit::Px)))
    {
        return Ok((
            vec![],
            Some(ManipulationBlock {
                axis,
                constraint_ids: ids,
                properties,
                reason: "a controlling equality uses an offset unit that cannot be preserved by direct manipulation".into(),
            }),
        ));
    }

    if !equalities.is_empty() {
        let edits = equalities
            .iter()
            .map(|constraint| ConstraintOffsetEdit {
                constraint_id: constraint.id.clone(),
                source_order: constraint.source_order,
                axis,
                previous_offset: constraint.offset,
                next_offset: constraint.offset + delta,
                unit: constraint.offset_unit,
            })
            .collect::<Vec<_>>();
        return Ok(
            match candidate_axis_motion_is_exact(scene, node, axis, delta, None, &edits, &ids) {
                Ok(()) => (edits, None),
                Err(reason) => (
                    vec![],
                    Some(ManipulationBlock {
                        axis,
                        constraint_ids: ids,
                        properties,
                        reason: format!("multi-constraint drag rejected: {reason}"),
                    }),
                ),
            },
        );
    }

    if node.component_origin.is_some() {
        return Ok((
            vec![],
            Some(ManipulationBlock {
                axis,
                constraint_ids: ids,
                properties,
                reason: "component inverse manipulation with inequality-only guards is not yet unambiguous".into(),
            }),
        ));
    }

    let direct_edit = ManipulationEdit {
        property: property.into(),
        value: moved_value(node, property, delta)?,
    };
    Ok(
        match candidate_axis_motion_is_exact(
            scene,
            node,
            axis,
            delta,
            Some(&direct_edit),
            &[],
            &ids,
        ) {
            Ok(()) => (vec![], None),
            Err(reason) => (
                vec![],
                Some(ManipulationBlock {
                    axis,
                    constraint_ids: ids,
                    properties,
                    reason: format!("guarded drag rejected: {reason}"),
                }),
            ),
        },
    )
}

pub fn plan_direct_manipulation(
    scene: &Scene,
    node_id: &str,
    dx: f64,
    dy: f64,
) -> Result<DirectManipulationPlan> {
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| anyhow!("node {node_id} not found"))?;

    if !node.provenance.authored || node.roles.iter().any(|role| role == "generated") {
        return Ok(DirectManipulationPlan {
            node_id: node.id.clone(),
            dx,
            dy,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: vec![],
            note: "node is generated; edit its procedural source instead".into(),
        });
    }

    let requested = [
        (ManipulationAxis::Horizontal, dx),
        (ManipulationAxis::Vertical, dy),
    ];
    let mut edits = Vec::new();
    let mut constraint_edits = Vec::new();
    let mut instance_parameter_edits = Vec::new();
    let mut blocked = Vec::new();
    for (axis, delta) in requested {
        if delta.abs() <= f64::EPSILON {
            continue;
        }
        let Some(property) = axis_property(node, axis) else {
            return Ok(DirectManipulationPlan {
                node_id: node.id.clone(),
                dx,
                dy,
                allowed: false,
                edits: vec![],
                constraint_edits: vec![],
                instance_parameter_edits: vec![],
                blocked,
                note: format!(
                    "{:?} nodes do not expose a direct positional authoring property",
                    node.kind
                ),
            });
        };
        let (axis_constraint_edits, block) =
            axis_constraint_resolution(scene, node, axis, property, delta)?;
        if let Some(block) = block {
            blocked.push(block);
            continue;
        }
        if !axis_constraint_edits.is_empty() {
            constraint_edits.extend(axis_constraint_edits);
            continue;
        }
        if node.component_origin.is_some() {
            let (parameter_edit, block) = component_parameter_edit(node, axis, property, delta)?;
            if let Some(block) = block {
                blocked.push(block);
                continue;
            }
            if let Some(parameter_edit) = parameter_edit {
                instance_parameter_edits.push(parameter_edit);
                continue;
            }
        }
        edits.push(ManipulationEdit {
            property: property.into(),
            value: moved_value(node, property, delta)?,
        });
    }

    let allowed = blocked.is_empty();
    let note = if !allowed {
        "drag blocked to preserve authored constraint intent".into()
    } else if !constraint_edits.is_empty() {
        "drag preserves authored relations by updating their offsets".into()
    } else if !instance_parameter_edits.is_empty() {
        "drag preserves component intent by updating instance parameters".into()
    } else {
        "drag can be represented as authored position edits without changing constraints".into()
    };
    Ok(DirectManipulationPlan {
        node_id: node.id.clone(),
        dx,
        dy,
        allowed,
        edits: if allowed { edits } else { vec![] },
        constraint_edits: if allowed { constraint_edits } else { vec![] },
        instance_parameter_edits: if allowed {
            instance_parameter_edits
        } else {
            vec![]
        },
        blocked,
        note,
    })
}
pub fn apply_direct_manipulation_source(
    source: &str,
    plan: &DirectManipulationPlan,
) -> Result<String> {
    if !plan.allowed {
        let blocked = plan
            .blocked
            .iter()
            .flat_map(|block| block.constraint_ids.iter().cloned())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!(
            "direct manipulation of {} is blocked by authored constraint intent{}",
            plan.node_id,
            if blocked.is_empty() {
                String::new()
            } else {
                format!(": {blocked}")
            }
        ));
    }
    let mut edited = source.to_string();
    for edit in &plan.edits {
        edited = set_source_property(&edited, &plan.node_id, &edit.property, edit.value.clone())?;
    }
    for edit in &plan.constraint_edits {
        edited =
            set_source_constraint_offset(&edited, edit.source_order, edit.next_offset, edit.unit)?;
    }
    for edit in &plan.instance_parameter_edits {
        edited = set_source_instance_parameter(
            &edited,
            &edit.instance_id,
            &edit.parameter,
            edit.next_value.clone(),
        )?;
    }
    Ok(edited)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeEdit {
    pub property: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectResizePlan {
    pub node_id: String,
    pub dw: f64,
    pub dh: f64,
    pub allowed: bool,
    pub edits: Vec<ResizeEdit>,
    pub constraint_edits: Vec<ConstraintOffsetEdit>,
    pub instance_parameter_edits: Vec<ComponentParameterEdit>,
    pub blocked: Vec<ManipulationBlock>,
    pub note: String,
}

fn authored_size_edit(
    node: &Node,
    axis: ManipulationAxis,
    visual_delta: f64,
) -> Option<(&'static str, f64)> {
    match (node.kind.clone(), axis) {
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Horizontal) => {
            Some(("width", visual_delta))
        }
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Vertical) => {
            Some(("height", visual_delta))
        }
        (NodeKind::Ellipse, ManipulationAxis::Horizontal) => Some(("rx", visual_delta / 2.0)),
        (NodeKind::Ellipse, ManipulationAxis::Vertical) => Some(("ry", visual_delta / 2.0)),
        _ => None,
    }
}

fn controls_size_axis(kind: &NodeKind, axis: ManipulationAxis, property: &str) -> bool {
    match (kind, axis) {
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Horizontal) => property == "width",
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Vertical) => property == "height",
        (NodeKind::Ellipse, ManipulationAxis::Horizontal) => matches!(property, "width" | "rx"),
        (NodeKind::Ellipse, ManipulationAxis::Vertical) => matches!(property, "height" | "ry"),
        _ => false,
    }
}

fn size_constraints<'a>(
    scene: &'a Scene,
    node: &Node,
    axis: ManipulationAxis,
) -> Vec<&'a Constraint> {
    scene
        .constraints
        .iter()
        .filter(|constraint| {
            constraint.lhs.node == node.id
                && controls_size_axis(&node.kind, axis, &constraint.lhs.property)
        })
        .collect()
}

fn resized_value(node: &Node, property: &str, delta: f64) -> Result<Value> {
    let value = moved_value(node, property, delta)?;
    let numeric = value
        .number()
        .ok_or_else(|| anyhow!("resize requires numeric {}.{}", node.id, property))?;
    if numeric <= 0.0 {
        return Err(anyhow!(
            "resize would make {}.{} non-positive ({numeric})",
            node.id,
            property
        ));
    }
    Ok(value)
}

fn node_number(node: &Node, property: &str, default: f64) -> f64 {
    node.derived
        .props
        .get(property)
        .copied()
        .or_else(|| node.authored.props.get(property).and_then(Value::number))
        .unwrap_or(default)
}

fn path_base_bounds(node: &Node) -> Result<[f64; 4]> {
    let d = node
        .authored
        .props
        .get("d")
        .and_then(Value::text)
        .ok_or_else(|| anyhow!("path {} has no authored d geometry", node.id))?;
    let path = BezPath::from_svg(d)
        .map_err(|error| anyhow!("path {} has invalid SVG geometry: {error}", node.id))?;
    if path.elements().is_empty() {
        return Err(anyhow!("path {} has empty geometry", node.id));
    }
    let bounds = path.bounding_box();
    Ok([bounds.x0, bounds.y0, bounds.width(), bounds.height()])
}

fn plan_path_resize(scene: &Scene, node: &Node, dw: f64, dh: f64) -> Result<DirectResizePlan> {
    let requested_axes = [
        (ManipulationAxis::Horizontal, dw),
        (ManipulationAxis::Vertical, dh),
    ]
    .into_iter()
    .filter(|(_, delta)| delta.abs() > f64::EPSILON)
    .collect::<Vec<_>>();
    if requested_axes.is_empty() {
        return Ok(DirectResizePlan {
            node_id: node.id.clone(),
            dw,
            dh,
            allowed: true,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: vec![],
            note: "resize has no effective delta".into(),
        });
    }
    let constraints = query::constraints_for(scene, &node.id);
    if !constraints.is_empty() {
        let ids = constraints
            .iter()
            .map(|constraint| constraint.id.clone())
            .collect::<Vec<_>>();
        return Ok(DirectResizePlan {
            node_id: node.id.clone(),
            dw,
            dh,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: requested_axes
                .iter()
                .map(|(axis, _)| ManipulationBlock {
                    axis: *axis,
                    constraint_ids: ids.clone(),
                    properties: vec!["affine_path_resize".into()],
                    reason: "path resize with authored constraints is not yet safe to invert"
                        .into(),
                })
                .collect(),
            note: "path resize blocked to preserve authored constraint intent".into(),
        });
    }
    if node.component_origin.is_some() {
        return Ok(DirectResizePlan {
            node_id: node.id.clone(),
            dw,
            dh,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: requested_axes
                .iter()
                .map(|(axis, _)| ManipulationBlock {
                    axis: *axis,
                    constraint_ids: vec![],
                    properties: vec!["affine_path_resize".into()],
                    reason: "component path resize requires an explicit inverse parameter binding"
                        .into(),
                })
                .collect(),
            note: "path resize blocked to preserve component intent".into(),
        });
    }
    let rotation = node_number(node, "rotate", node_number(node, "rotation", 0.0));
    if rotation.abs() > 0.0001 {
        return Ok(DirectResizePlan {
            node_id: node.id.clone(),
            dw,
            dh,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: requested_axes
                .iter()
                .map(|(axis, _)| ManipulationBlock {
                    axis: *axis,
                    constraint_ids: vec![],
                    properties: vec!["rotate".into()],
                    reason: "axis-aligned path resize does not guess through an existing rotation"
                        .into(),
                })
                .collect(),
            note: "path resize blocked because rotated affine intent is ambiguous".into(),
        });
    }

    let base = path_base_bounds(node)?;
    let before =
        query::bounds(node).ok_or_else(|| anyhow!("path {} has no measurable bounds", node.id))?;
    let scale = node_number(node, "scale", 1.0);
    let mut edits = Vec::new();
    for (axis, delta) in requested_axes {
        let (
            extent_index,
            base_min,
            base_extent,
            scale_property,
            translate_property,
            origin_property,
        ) = match axis {
            ManipulationAxis::Horizontal => {
                (2, base[0], base[2], "scale_x", "translate_x", "origin_x")
            }
            ManipulationAxis::Vertical => {
                (3, base[1], base[3], "scale_y", "translate_y", "origin_y")
            }
        };
        let current_extent = before[extent_index];
        let target_extent = current_extent + delta;
        if !current_extent.is_finite()
            || current_extent <= 0.0
            || !base_extent.is_finite()
            || base_extent <= 0.0
        {
            return Err(anyhow!(
                "path {} has a degenerate {:?} extent",
                node.id,
                axis
            ));
        }
        if !target_extent.is_finite() || target_extent <= 0.0 {
            return Err(anyhow!(
                "resize would make path {} {:?} extent non-positive ({target_extent})",
                node.id,
                axis
            ));
        }
        let current_scale = node_number(node, scale_property, scale);
        if !current_scale.is_finite() || current_scale <= 0.0 {
            return Err(anyhow!(
                "path {} has unsupported non-positive {} {}",
                node.id,
                scale_property,
                current_scale
            ));
        }
        let next_scale = current_scale * target_extent / current_extent;
        let origin = node_number(node, origin_property, 0.0);
        let anchor = match axis {
            ManipulationAxis::Horizontal => before[0],
            ManipulationAxis::Vertical => before[1],
        };
        let next_translate = anchor - origin - (base_min - origin) * next_scale;
        edits.push(ResizeEdit {
            property: scale_property.into(),
            value: Value::Number(next_scale),
        });
        edits.push(ResizeEdit {
            property: translate_property.into(),
            value: Value::Number(next_translate),
        });
    }

    let mut candidate = node.clone();
    for edit in &edits {
        candidate
            .authored
            .props
            .insert(edit.property.clone(), edit.value.clone());
        if let Some(value) = edit.value.number() {
            candidate.derived.props.insert(edit.property.clone(), value);
        }
    }
    let after = query::bounds(&candidate)
        .ok_or_else(|| anyhow!("path {} became unmeasurable after resize", node.id))?;
    if (after[0] - before[0]).abs() > 0.0001
        || (after[1] - before[1]).abs() > 0.0001
        || (after[2] - (before[2] + dw)).abs() > 0.0001
        || (after[3] - (before[3] + dh)).abs() > 0.0001
    {
        return Err(anyhow!(
            "path {} affine resize failed exact visual validation",
            node.id
        ));
    }
    Ok(DirectResizePlan {
        node_id: node.id.clone(),
        dw,
        dh,
        allowed: true,
        edits,
        constraint_edits: vec![],
        instance_parameter_edits: vec![],
        blocked: vec![],
        note: "path resize preserves d geometry through explicit affine scale/translation".into(),
    })
}

fn size_constraint_delta(
    kind: &NodeKind,
    axis: ManipulationAxis,
    property: &str,
    visual_delta: f64,
) -> Option<f64> {
    match (kind, axis, property) {
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Horizontal, "width")
        | (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Vertical, "height")
        | (NodeKind::Ellipse, ManipulationAxis::Horizontal, "width")
        | (NodeKind::Ellipse, ManipulationAxis::Vertical, "height") => Some(visual_delta),
        (NodeKind::Ellipse, ManipulationAxis::Horizontal, "rx")
        | (NodeKind::Ellipse, ManipulationAxis::Vertical, "ry") => Some(visual_delta / 2.0),
        _ => None,
    }
}

fn size_axis_ref(node: &Node, axis: ManipulationAxis) -> Option<PropertyRef> {
    let property = match (&node.kind, axis) {
        (NodeKind::Rect | NodeKind::Image | NodeKind::Ellipse, ManipulationAxis::Horizontal) => {
            "width"
        }
        (NodeKind::Rect | NodeKind::Image | NodeKind::Ellipse, ManipulationAxis::Vertical) => {
            "height"
        }
        _ => return None,
    };
    Some(PropertyRef {
        node: node.id.clone(),
        property: property.into(),
    })
}

fn candidate_axis_resize_is_exact(
    scene: &Scene,
    node: &Node,
    axis: ManipulationAxis,
    visual_delta: f64,
    direct_edit: Option<&ResizeEdit>,
    constraint_edits: &[ConstraintOffsetEdit],
    controlling_ids: &[String],
) -> Result<(), String> {
    let Some(reference) = size_axis_ref(node, axis) else {
        return Err("node has no measurable size for this axis".into());
    };
    let mut baseline = scene.clone();
    let baseline_report = solve_scene(&mut baseline);
    if !baseline_report.required_conflicts.is_empty() || !baseline_report.unresolved.is_empty() {
        return Err("baseline scene is not fully solvable".into());
    }
    let before = property_value(&baseline, &reference)
        .ok_or_else(|| format!("cannot measure {}.{}", reference.node, reference.property))?;

    let mut candidate = baseline.clone();
    if let Some(edit) = direct_edit {
        let target = candidate
            .nodes
            .iter_mut()
            .find(|candidate_node| candidate_node.id == node.id)
            .ok_or_else(|| format!("candidate node {} disappeared", node.id))?;
        target
            .authored
            .props
            .insert(edit.property.clone(), edit.value.clone());
    }
    for edit in constraint_edits {
        let constraint = candidate
            .constraints
            .iter_mut()
            .find(|constraint| constraint.source_order == edit.source_order)
            .ok_or_else(|| format!("{} disappeared from candidate scene", edit.constraint_id))?;
        constraint.offset = edit.next_offset;
        constraint.offset_unit = edit.unit;
    }

    let report = solve_scene(&mut candidate);
    if !report.required_conflicts.is_empty() || !report.unresolved.is_empty() {
        return Err(
            "candidate edit introduces an unresolved or required constraint conflict".into(),
        );
    }
    if report
        .relaxed
        .iter()
        .any(|id| controlling_ids.iter().any(|control| control == id))
    {
        return Err("candidate edit would relax a controlling size constraint".into());
    }
    if report.relaxed.iter().any(|id| {
        !baseline_report
            .relaxed
            .iter()
            .any(|baseline_id| baseline_id == id)
    }) {
        return Err("candidate edit would newly relax another authored constraint".into());
    }
    let after = property_value(&candidate, &reference).ok_or_else(|| {
        format!(
            "cannot re-measure {}.{}",
            reference.node, reference.property
        )
    })?;
    if after <= 0.0 {
        return Err(format!(
            "candidate edit resolves to non-positive size {after:.4}"
        ));
    }
    let actual = after - before;
    if (actual - visual_delta).abs() > 0.0001 {
        return Err(format!(
            "candidate edit resolves to delta {actual:.4}, not requested delta {visual_delta:.4}"
        ));
    }
    Ok(())
}

fn size_constraint_resolution(
    scene: &Scene,
    node: &Node,
    axis: ManipulationAxis,
    property: &str,
    authored_delta: f64,
    visual_delta: f64,
) -> Result<(Vec<ConstraintOffsetEdit>, Option<ManipulationBlock>)> {
    let constraints = size_constraints(scene, node, axis);
    if constraints.is_empty() {
        return Ok((vec![], None));
    }
    let ids = constraints
        .iter()
        .map(|constraint| constraint.id.clone())
        .collect::<Vec<_>>();
    let properties = constraints
        .iter()
        .map(|constraint| constraint.lhs.property.clone())
        .collect::<Vec<_>>();
    let equalities = constraints
        .iter()
        .copied()
        .filter(|constraint| constraint.relation == Relation::Eq)
        .collect::<Vec<_>>();

    if equalities
        .iter()
        .any(|constraint| !matches!(constraint.offset_unit, None | Some(Unit::Px)))
    {
        return Ok((
            vec![],
            Some(ManipulationBlock {
                axis,
                constraint_ids: ids,
                properties,
                reason: "a controlling size equality uses an offset unit that cannot be preserved by direct resize".into(),
            }),
        ));
    }

    if !equalities.is_empty() {
        let mut edits = Vec::with_capacity(equalities.len());
        for constraint in equalities {
            let Some(delta) =
                size_constraint_delta(&node.kind, axis, &constraint.lhs.property, visual_delta)
            else {
                return Ok((
                    vec![],
                    Some(ManipulationBlock {
                        axis,
                        constraint_ids: ids,
                        properties,
                        reason: "a controlling size equality cannot be mapped to the requested visual dimension".into(),
                    }),
                ));
            };
            edits.push(ConstraintOffsetEdit {
                constraint_id: constraint.id.clone(),
                source_order: constraint.source_order,
                axis,
                previous_offset: constraint.offset,
                next_offset: constraint.offset + delta,
                unit: constraint.offset_unit,
            });
        }
        return Ok(
            match candidate_axis_resize_is_exact(
                scene,
                node,
                axis,
                visual_delta,
                None,
                &edits,
                &ids,
            ) {
                Ok(()) => (edits, None),
                Err(reason) => (
                    vec![],
                    Some(ManipulationBlock {
                        axis,
                        constraint_ids: ids,
                        properties,
                        reason: format!("multi-constraint resize rejected: {reason}"),
                    }),
                ),
            },
        );
    }

    if node.component_origin.is_some() {
        return Ok((
            vec![],
            Some(ManipulationBlock {
                axis,
                constraint_ids: ids,
                properties,
                reason:
                    "component inverse resize with inequality-only guards is not yet unambiguous"
                        .into(),
            }),
        ));
    }

    let direct_edit = ResizeEdit {
        property: property.into(),
        value: resized_value(node, property, authored_delta)?,
    };
    Ok(
        match candidate_axis_resize_is_exact(
            scene,
            node,
            axis,
            visual_delta,
            Some(&direct_edit),
            &[],
            &ids,
        ) {
            Ok(()) => (vec![], None),
            Err(reason) => (
                vec![],
                Some(ManipulationBlock {
                    axis,
                    constraint_ids: ids,
                    properties,
                    reason: format!("guarded resize rejected: {reason}"),
                }),
            ),
        },
    )
}

pub fn plan_direct_resize(
    scene: &Scene,
    node_id: &str,
    dw: f64,
    dh: f64,
) -> Result<DirectResizePlan> {
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| anyhow!("node {node_id} not found"))?;
    if !node.provenance.authored || node.roles.iter().any(|r| r == "generated") {
        return Ok(DirectResizePlan {
            node_id: node.id.clone(),
            dw,
            dh,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            blocked: vec![],
            note: "node is generated; edit its procedural source instead".into(),
        });
    }
    if node.kind == NodeKind::Path {
        return plan_path_resize(scene, node, dw, dh);
    }
    let requested = [
        (ManipulationAxis::Horizontal, dw),
        (ManipulationAxis::Vertical, dh),
    ];
    let mut edits = Vec::new();
    let mut constraint_edits = Vec::new();
    let mut instance_parameter_edits = Vec::new();
    let mut blocked = Vec::new();
    for (axis, delta) in requested {
        if delta.abs() <= f64::EPSILON {
            continue;
        }
        let Some((property, authored_delta)) = authored_size_edit(node, axis, delta) else {
            return Ok(DirectResizePlan {
                node_id: node.id.clone(),
                dw,
                dh,
                allowed: false,
                edits: vec![],
                constraint_edits: vec![],
                instance_parameter_edits: vec![],
                blocked,
                note: format!(
                    "{:?} nodes do not expose direct semantic resize properties",
                    node.kind
                ),
            });
        };
        let (axis_constraint_edits, block) =
            size_constraint_resolution(scene, node, axis, property, authored_delta, delta)?;
        if let Some(block) = block {
            blocked.push(block);
            continue;
        }
        if !axis_constraint_edits.is_empty() {
            constraint_edits.extend(axis_constraint_edits);
            continue;
        }
        if node.component_origin.is_some() {
            let (parameter_edit, block) =
                component_parameter_edit(node, axis, property, authored_delta)?;
            if let Some(block) = block {
                blocked.push(block);
                continue;
            }
            if let Some(parameter_edit) = parameter_edit {
                let next = parameter_edit
                    .next_value
                    .number()
                    .ok_or_else(|| anyhow!("component resize parameter is not numeric"))?;
                if next <= 0.0 {
                    return Err(anyhow!(
                        "resize would make component parameter {} non-positive ({next})",
                        parameter_edit.parameter
                    ));
                }
                instance_parameter_edits.push(parameter_edit);
                continue;
            }
        }
        edits.push(ResizeEdit {
            property: property.into(),
            value: resized_value(node, property, authored_delta)?,
        });
    }
    let allowed = blocked.is_empty();
    let note = if !allowed {
        "resize blocked to preserve authored size constraint intent".into()
    } else if !constraint_edits.is_empty() {
        "resize preserves authored size relations by updating their offsets".into()
    } else if !instance_parameter_edits.is_empty() {
        "resize preserves component intent by updating instance parameters".into()
    } else {
        "resize can be represented as authored width/height edits".into()
    };
    Ok(DirectResizePlan {
        node_id: node.id.clone(),
        dw,
        dh,
        allowed,
        edits: if allowed { edits } else { vec![] },
        constraint_edits: if allowed { constraint_edits } else { vec![] },
        instance_parameter_edits: if allowed {
            instance_parameter_edits
        } else {
            vec![]
        },
        blocked,
        note,
    })
}

pub fn apply_direct_resize_source(source: &str, plan: &DirectResizePlan) -> Result<String> {
    if !plan.allowed {
        let blocked = plan
            .blocked
            .iter()
            .flat_map(|block| block.constraint_ids.iter().cloned())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(anyhow!(
            "direct resize of {} is blocked by authored constraint intent{}",
            plan.node_id,
            if blocked.is_empty() {
                String::new()
            } else {
                format!(": {blocked}")
            }
        ));
    }
    let mut edited = source.to_string();
    for edit in &plan.edits {
        edited = set_source_property(&edited, &plan.node_id, &edit.property, edit.value.clone())?;
    }
    for edit in &plan.constraint_edits {
        edited =
            set_source_constraint_offset(&edited, edit.source_order, edit.next_offset, edit.unit)?;
    }
    for edit in &plan.instance_parameter_edits {
        edited = set_source_instance_parameter(
            &edited,
            &edit.instance_id,
            &edit.parameter,
            edit.next_value.clone(),
        )?;
    }
    Ok(edited)
}
