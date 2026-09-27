use crate::{
    model::{
        Constraint, ConstraintRhs, Dimension, Node, NodeKind, PropertyRef, Quantity, Relation,
        Scene, Unit, Value, property_dimension,
    },
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub member_plans: Vec<DirectManipulationPlan>,
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
        "translate_x" if node.kind == NodeKind::Path => {
            let local_origin = node_number(node, "origin_x", 0.0);
            (transformed - origin.offset_x - (origin.scale - 1.0) * local_origin) / origin.scale
        }
        "translate_y" if node.kind == NodeKind::Path => {
            let local_origin = node_number(node, "origin_y", 0.0);
            (transformed - origin.offset_y - (origin.scale - 1.0) * local_origin) / origin.scale
        }
        "scale" | "scale_x" | "scale_y" if node.kind == NodeKind::Path => {
            transformed / origin.scale
        }
        "origin_x" | "origin_y" if node.kind == NodeKind::Path => transformed,
        _ if property_dimension(property) == Some(Dimension::Length) => transformed / origin.scale,
        _ => transformed,
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
    let local_delta = match property {
        "x" | "cx" | "y" | "cy" => delta / origin.scale,
        "translate_x" | "translate_y" if node.kind == NodeKind::Path => delta / origin.scale,
        "scale" | "scale_x" | "scale_y" if node.kind == NodeKind::Path => delta / origin.scale,
        "origin_x" | "origin_y" if node.kind == NodeKind::Path => delta,
        _ if property_dimension(property) == Some(Dimension::Length) => delta / origin.scale,
        _ => delta,
    };
    let next_number = previous_value
        .number()
        .ok_or_else(|| anyhow!("component parameter {} is not numeric", bindings[0]))?
        + local_delta;
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

fn group_leaf_ids(scene: &Scene, group: &Node) -> Result<Vec<String>> {
    let mut leaves = Vec::new();
    fn visit(scene: &Scene, node: &Node, leaves: &mut Vec<String>) -> Result<()> {
        if node.kind != NodeKind::Group {
            leaves.push(node.id.clone());
            return Ok(());
        }
        for member_id in &node.members {
            let member = scene
                .nodes
                .iter()
                .find(|candidate| candidate.id == *member_id)
                .ok_or_else(|| {
                    anyhow!("group {} references missing member {member_id}", node.id)
                })?;
            visit(scene, member, leaves)?;
        }
        Ok(())
    }
    visit(scene, group, &mut leaves)?;
    Ok(leaves)
}

fn plan_group_translation(
    scene: &Scene,
    group: &Node,
    dx: f64,
    dy: f64,
) -> Result<DirectManipulationPlan> {
    let leaves = group_leaf_ids(scene, group)?;
    if leaves.is_empty() {
        return Ok(DirectManipulationPlan {
            node_id: group.id.clone(),
            dx,
            dy,
            allowed: false,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            member_plans: vec![],
            blocked: vec![ManipulationBlock {
                axis: if dx.abs() > f64::EPSILON {
                    ManipulationAxis::Horizontal
                } else {
                    ManipulationAxis::Vertical
                },
                constraint_ids: vec![],
                properties: vec!["members".into()],
                reason: "empty structural groups have no geometry to translate".into(),
            }],
            note: "group drag blocked because the group has no members".into(),
        });
    }
    if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
        return Ok(DirectManipulationPlan {
            node_id: group.id.clone(),
            dx,
            dy,
            allowed: true,
            edits: vec![],
            constraint_edits: vec![],
            instance_parameter_edits: vec![],
            member_plans: vec![],
            blocked: vec![],
            note: "group translation is a no-op".into(),
        });
    }

    let before = leaves
        .iter()
        .map(|id| {
            let node = scene
                .nodes
                .iter()
                .find(|candidate| candidate.id == *id)
                .ok_or_else(|| anyhow!("group member {id} disappeared"))?;
            let bounds = query::bounds(node)
                .ok_or_else(|| anyhow!("group member {id} has no measurable bounds"))?;
            Ok((id.clone(), bounds))
        })
        .collect::<Result<Vec<_>>>()?;

    // Relations between two members of the same group are translation invariant when both
    // referenced properties move on the same axis. Plan leaf source edits without those
    // internal relations, then validate the fully composed edit against the original scene.
    let mut planning_scene = scene.clone();
    planning_scene.constraints.retain(|constraint| {
        let ConstraintRhs::Property(rhs) = &constraint.rhs else {
            return true;
        };
        if !leaves.iter().any(|id| id == &constraint.lhs.node)
            || !leaves.iter().any(|id| id == &rhs.node)
        {
            return true;
        }
        let horizontal =
            positional_property(&constraint.lhs.property, ManipulationAxis::Horizontal)
                && positional_property(&rhs.property, ManipulationAxis::Horizontal);
        let vertical = positional_property(&constraint.lhs.property, ManipulationAxis::Vertical)
            && positional_property(&rhs.property, ManipulationAxis::Vertical);
        !(horizontal || vertical)
    });

    let mut member_plans = Vec::new();
    for member_id in &leaves {
        let plan = plan_direct_manipulation(&planning_scene, member_id, dx, dy)?;
        if !plan.allowed {
            let mut blocked = plan.blocked.clone();
            if blocked.is_empty() {
                blocked.push(ManipulationBlock {
                    axis: if dx.abs() > f64::EPSILON {
                        ManipulationAxis::Horizontal
                    } else {
                        ManipulationAxis::Vertical
                    },
                    constraint_ids: vec![],
                    properties: vec![member_id.clone()],
                    reason: format!(
                        "group member {member_id} cannot preserve authored drag intent"
                    ),
                });
            } else {
                for block in &mut blocked {
                    block.reason = format!("group member {member_id}: {}", block.reason);
                }
            }
            member_plans.push(plan);
            return Ok(DirectManipulationPlan {
                node_id: group.id.clone(),
                dx,
                dy,
                allowed: false,
                edits: vec![],
                constraint_edits: vec![],
                instance_parameter_edits: vec![],
                member_plans,
                blocked,
                note: "group drag blocked because one member cannot be translated safely".into(),
            });
        }
        member_plans.push(plan);
    }

    let mut candidate = scene.clone();
    for plan in &member_plans {
        apply_movement_plan_to_candidate(&mut candidate, plan)?;
    }

    for (member_id, [x, y, width, height]) in before {
        let node = candidate
            .nodes
            .iter()
            .find(|candidate| candidate.id == member_id)
            .ok_or_else(|| anyhow!("group member {member_id} disappeared after candidate drag"))?;
        let after = query::bounds(node).ok_or_else(|| {
            anyhow!("group member {member_id} became unmeasurable after candidate drag")
        })?;
        let expected = [x + dx, y + dy, width, height];
        if after
            .iter()
            .zip(expected.iter())
            .any(|(actual, expected)| (actual - expected).abs() > 0.0001)
        {
            return Ok(DirectManipulationPlan {
                node_id: group.id.clone(),
                dx,
                dy,
                allowed: false,
                edits: vec![],
                constraint_edits: vec![],
                instance_parameter_edits: vec![],
                member_plans,
                blocked: vec![ManipulationBlock {
                    axis: if (after[0] - expected[0]).abs() > 0.0001 {
                        ManipulationAxis::Horizontal
                    } else {
                        ManipulationAxis::Vertical
                    },
                    constraint_ids: vec![],
                    properties: vec![member_id.clone(), "bounds".into()],
                    reason: format!(
                        "group member {member_id} resolved to {after:?}, not requested {expected:?}"
                    ),
                }],
                note: "group drag failed exact member-translation validation".into(),
            });
        }
    }

    Ok(DirectManipulationPlan {
        node_id: group.id.clone(),
        dx,
        dy,
        allowed: true,
        edits: vec![],
        constraint_edits: vec![],
        instance_parameter_edits: vec![],
        member_plans,
        blocked: vec![],
        note: "group drag preserves structural membership by translating every leaf member through its semantic source contract".into(),
    })
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
            member_plans: vec![],
            blocked: vec![],
            note: "node is generated; edit its procedural source instead".into(),
        });
    }
    if node.kind == NodeKind::Group {
        return plan_group_translation(scene, node, dx, dy);
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
                member_plans: vec![],
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
        member_plans: vec![],
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
    for member_plan in &plan.member_plans {
        edited = apply_direct_manipulation_source(&edited, member_plan)?;
    }
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ResizeHandle {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeHandle {
    fn moves_left(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

impl std::str::FromStr for ResizeHandle {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "top-left" | "top_left" => Ok(Self::TopLeft),
            "top-right" | "top_right" => Ok(Self::TopRight),
            "bottom-left" | "bottom_left" => Ok(Self::BottomLeft),
            "bottom-right" | "bottom_right" => Ok(Self::BottomRight),
            _ => Err(format!(
                "unknown resize handle {value:?}; expected left, right, top, bottom, top-left, top-right, bottom-left, or bottom-right"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectHandleResizePlan {
    pub node_id: String,
    pub handle: ResizeHandle,
    pub dx: f64,
    pub dy: f64,
    pub target_bounds: [f64; 4],
    pub allowed: bool,
    pub resize_plan: Option<DirectResizePlan>,
    pub movement_plan: Option<DirectManipulationPlan>,
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
        (NodeKind::Text, ManipulationAxis::Horizontal)
            if node.authored.props.contains_key("max_width") =>
        {
            Some(("max_width", visual_delta))
        }
        _ => None,
    }
}

fn controls_size_axis(kind: &NodeKind, axis: ManipulationAxis, property: &str) -> bool {
    match (kind, axis) {
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Horizontal) => property == "width",
        (NodeKind::Rect | NodeKind::Image, ManipulationAxis::Vertical) => property == "height",
        (NodeKind::Ellipse, ManipulationAxis::Horizontal) => matches!(property, "width" | "rx"),
        (NodeKind::Ellipse, ManipulationAxis::Vertical) => matches!(property, "height" | "ry"),
        (NodeKind::Text, ManipulationAxis::Horizontal) => property == "max_width",
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
    let unsafe_constraints = constraints
        .iter()
        .copied()
        .filter(|constraint| {
            if constraint.lhs.node == node.id {
                !matches!(
                    constraint.lhs.property.as_str(),
                    "translate_x" | "translate_y"
                )
            } else {
                matches!(
                    &constraint.rhs,
                    ConstraintRhs::Property(reference) if reference.node == node.id
                )
            }
        })
        .collect::<Vec<_>>();
    if !unsafe_constraints.is_empty() {
        let ids = unsafe_constraints
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
                    reason: "path resize only inverts authored constraints that directly control translate_x/translate_y; scale/geometry constraints and RHS dependencies remain ambiguous".into(),
                })
                .collect(),
            note: "path resize blocked to preserve unsupported constraint intent".into(),
        });
    }
    if node.component_origin.is_some() && !constraints.is_empty() {
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
                    reason: "component-owned path resize does not combine parameter inversion with authored path constraints; remove the ambiguity or edit the component explicitly".into(),
                })
                .collect(),
            note: "component path resize blocked because parameter and constraint ownership overlap"
                .into(),
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
    let component_owned = node.component_origin.is_some();
    let mut edits = Vec::new();
    let mut constraint_edits = Vec::new();
    let mut instance_parameter_edits = Vec::new();
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
        let current_translate = node_number(node, translate_property, 0.0);
        let next_translate = anchor - origin - (base_min - origin) * next_scale;
        let translation_delta = next_translate - current_translate;
        if component_owned {
            let (scale_parameter, scale_block) =
                component_parameter_edit(node, axis, scale_property, next_scale - current_scale)?;
            if let Some(block) = scale_block {
                return Ok(DirectResizePlan {
                    node_id: node.id.clone(),
                    dw,
                    dh,
                    allowed: false,
                    edits: vec![],
                    constraint_edits: vec![],
                    instance_parameter_edits: vec![],
                    blocked: vec![block],
                    note: "component path resize requires an unambiguous scale parameter binding"
                        .into(),
                });
            }
            if let Some(parameter) = scale_parameter {
                instance_parameter_edits.push(parameter);
            }
            let (translate_parameter, translate_block) =
                component_parameter_edit(node, axis, translate_property, translation_delta)?;
            if let Some(block) = translate_block {
                return Ok(DirectResizePlan {
                    node_id: node.id.clone(),
                    dw,
                    dh,
                    allowed: false,
                    edits: vec![],
                    constraint_edits: vec![],
                    instance_parameter_edits: vec![],
                    blocked: vec![block],
                    note: "component path resize requires an unambiguous translation parameter binding"
                        .into(),
                });
            }
            if let Some(parameter) = translate_parameter {
                instance_parameter_edits.push(parameter);
            }
        } else {
            edits.push(ResizeEdit {
                property: scale_property.into(),
                value: Value::Number(next_scale),
            });

            let (axis_constraint_edits, block) = axis_constraint_resolution(
                scene,
                node,
                axis,
                translate_property,
                translation_delta,
            )?;
            if let Some(block) = block {
                return Ok(DirectResizePlan {
                    node_id: node.id.clone(),
                    dw,
                    dh,
                    allowed: false,
                    edits: vec![],
                    constraint_edits: vec![],
                    instance_parameter_edits: vec![],
                    blocked: vec![block],
                    note: "path resize blocked because the required affine translation cannot preserve authored constraints".into(),
                });
            }
            if axis_constraint_edits.is_empty() {
                edits.push(ResizeEdit {
                    property: translate_property.into(),
                    value: Value::Number(next_translate),
                });
            } else {
                constraint_edits.extend(axis_constraint_edits);
            }
        }
    }

    let plan = DirectResizePlan {
        node_id: node.id.clone(),
        dw,
        dh,
        allowed: true,
        edits,
        constraint_edits,
        instance_parameter_edits,
        blocked: vec![],
        note: if component_owned {
            "component path resize preserves d geometry by inverting explicit affine parameter bindings"
                .into()
        } else {
            "path resize preserves d geometry through affine scale plus source-authoritative translation or relation-offset edits".into()
        },
    };
    let mut candidate_scene = scene.clone();
    apply_resize_plan_to_candidate(&mut candidate_scene, &plan)?;
    let candidate = candidate_scene
        .nodes
        .iter()
        .find(|candidate| candidate.id == node.id)
        .ok_or_else(|| anyhow!("path {} disappeared after resize", node.id))?;
    let after = query::bounds(candidate)
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
    Ok(plan)
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
        (NodeKind::Text, ManipulationAxis::Horizontal, "max_width") => Some(visual_delta),
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
        (NodeKind::Text, ManipulationAxis::Horizontal) => "max_width",
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
    if node.kind == NodeKind::Text {
        let mut text_blocks = Vec::new();
        if dh.abs() > f64::EPSILON {
            text_blocks.push(ManipulationBlock {
                axis: ManipulationAxis::Vertical,
                constraint_ids: vec![],
                properties: vec!["line_height".into(), "size".into()],
                reason: "vertical text resize is ambiguous between font scaling and line-spacing changes; Glyph only resizes an explicit wrap frame horizontally".into(),
            });
        }
        if dw.abs() > f64::EPSILON && !node.authored.props.contains_key("max_width") {
            text_blocks.push(ManipulationBlock {
                axis: ManipulationAxis::Horizontal,
                constraint_ids: vec![],
                properties: vec!["max_width".into()],
                reason: "point text has no authored max_width frame; add max_width before resizing text width".into(),
            });
        }
        let rotation = node_number(node, "rotate", node_number(node, "rotation", 0.0));
        let scale = node_number(node, "scale", 1.0);
        let scale_x = node_number(node, "scale_x", scale);
        if dw.abs() > f64::EPSILON && (rotation.abs() > 0.0001 || (scale_x - 1.0).abs() > 0.0001) {
            text_blocks.push(ManipulationBlock {
                axis: ManipulationAxis::Horizontal,
                constraint_ids: vec![],
                properties: vec!["max_width".into(), "scale_x".into(), "rotate".into()],
                reason: "text frame resize through an authored affine transform is not yet invertible without changing visual-space semantics".into(),
            });
        }
        if !text_blocks.is_empty() {
            return Ok(DirectResizePlan {
                node_id: node.id.clone(),
                dw,
                dh,
                allowed: false,
                edits: vec![],
                constraint_edits: vec![],
                instance_parameter_edits: vec![],
                blocked: text_blocks,
                note: "text resize blocked to preserve explicit text-layout intent".into(),
            });
        }
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

fn target_bounds_for_handle(
    before: [f64; 4],
    handle: ResizeHandle,
    dx: f64,
    dy: f64,
) -> Result<[f64; 4]> {
    let [mut x, mut y, mut width, mut height] = before;
    if handle.moves_left() {
        x += dx;
        width -= dx;
    } else if handle.moves_right() {
        width += dx;
    }
    if handle.moves_top() {
        y += dy;
        height -= dy;
    } else if handle.moves_bottom() {
        height += dy;
    }
    if ![x, y, width, height].iter().all(|value| value.is_finite()) {
        return Err(anyhow!("resize handle produced non-finite target bounds"));
    }
    if width <= 0.0 || height <= 0.0 {
        return Err(anyhow!(
            "resize handle would make visual bounds non-positive ({width} × {height})"
        ));
    }
    Ok([x, y, width, height])
}

fn set_candidate_component_target(
    scene: &mut Scene,
    node_id: &str,
    edit: &ComponentParameterEdit,
) -> Result<()> {
    let node = scene
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or_else(|| anyhow!("candidate node {node_id} disappeared"))?;
    let origin = node
        .component_origin
        .as_ref()
        .ok_or_else(|| anyhow!("candidate component node {node_id} lost origin metadata"))?;
    let instance_scale = origin.scale;
    let offset_x = origin.offset_x;
    let offset_y = origin.offset_y;
    let local = edit
        .next_value
        .number()
        .ok_or_else(|| anyhow!("component parameter {} is not numeric", edit.parameter))?;
    let global = match edit.target_property.as_str() {
        "x" | "cx" => local * instance_scale + offset_x,
        "y" | "cy" => local * instance_scale + offset_y,
        "translate_x" if node.kind == NodeKind::Path => {
            let local_origin = node_number(node, "origin_x", 0.0);
            offset_x + local * instance_scale + (instance_scale - 1.0) * local_origin
        }
        "translate_y" if node.kind == NodeKind::Path => {
            let local_origin = node_number(node, "origin_y", 0.0);
            offset_y + local * instance_scale + (instance_scale - 1.0) * local_origin
        }
        "scale" | "scale_x" | "scale_y" if node.kind == NodeKind::Path => local * instance_scale,
        "origin_x" | "origin_y" if node.kind == NodeKind::Path => local,
        _ if property_dimension(&edit.target_property) == Some(Dimension::Length) => {
            local * instance_scale
        }
        _ => local,
    };
    let value = match node.authored.props.get(&edit.target_property) {
        Some(Value::Quantity(quantity)) => Value::Quantity(Quantity {
            value: global,
            unit: quantity.unit,
        }),
        _ => Value::Number(global),
    };
    node.authored
        .props
        .insert(edit.target_property.clone(), value);
    Ok(())
}

fn apply_resize_plan_to_candidate(scene: &mut Scene, plan: &DirectResizePlan) -> Result<()> {
    if !plan.allowed {
        return Err(anyhow!(
            "cannot apply blocked resize plan to candidate scene"
        ));
    }
    {
        let node = scene
            .nodes
            .iter_mut()
            .find(|node| node.id == plan.node_id)
            .ok_or_else(|| anyhow!("candidate node {} disappeared", plan.node_id))?;
        for edit in &plan.edits {
            node.authored
                .props
                .insert(edit.property.clone(), edit.value.clone());
        }
    }
    for edit in &plan.constraint_edits {
        let constraint = scene
            .constraints
            .iter_mut()
            .find(|constraint| constraint.source_order == edit.source_order)
            .ok_or_else(|| anyhow!("{} disappeared from candidate scene", edit.constraint_id))?;
        constraint.offset = edit.next_offset;
        constraint.offset_unit = edit.unit;
    }
    for edit in &plan.instance_parameter_edits {
        set_candidate_component_target(scene, &plan.node_id, edit)?;
    }
    let report = solve_scene(scene);
    if !report.required_conflicts.is_empty() || !report.unresolved.is_empty() {
        return Err(anyhow!(
            "candidate resize introduced unresolved or required constraint conflicts"
        ));
    }
    Ok(())
}

fn apply_movement_plan_to_candidate(
    scene: &mut Scene,
    plan: &DirectManipulationPlan,
) -> Result<()> {
    if !plan.allowed {
        return Err(anyhow!(
            "cannot apply blocked movement plan to candidate scene"
        ));
    }
    {
        let node = scene
            .nodes
            .iter_mut()
            .find(|node| node.id == plan.node_id)
            .ok_or_else(|| anyhow!("candidate node {} disappeared", plan.node_id))?;
        for edit in &plan.edits {
            node.authored
                .props
                .insert(edit.property.clone(), edit.value.clone());
        }
    }
    for edit in &plan.constraint_edits {
        let constraint = scene
            .constraints
            .iter_mut()
            .find(|constraint| constraint.source_order == edit.source_order)
            .ok_or_else(|| anyhow!("{} disappeared from candidate scene", edit.constraint_id))?;
        constraint.offset = edit.next_offset;
        constraint.offset_unit = edit.unit;
    }
    for edit in &plan.instance_parameter_edits {
        set_candidate_component_target(scene, &plan.node_id, edit)?;
    }
    let report = solve_scene(scene);
    if !report.required_conflicts.is_empty() || !report.unresolved.is_empty() {
        return Err(anyhow!(
            "candidate movement introduced unresolved or required constraint conflicts"
        ));
    }
    Ok(())
}

pub fn plan_direct_resize_from_handle(
    scene: &Scene,
    node_id: &str,
    handle: ResizeHandle,
    dx: f64,
    dy: f64,
) -> Result<DirectHandleResizePlan> {
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| anyhow!("node {node_id} not found"))?;
    let before = query::bounds(node)
        .ok_or_else(|| anyhow!("node {node_id} has no measurable bounds for handle resize"))?;
    let target = target_bounds_for_handle(before, handle, dx, dy)?;
    let dw = target[2] - before[2];
    let dh = target[3] - before[3];
    let size_plan = plan_direct_resize(scene, node_id, dw, dh)?;
    if !size_plan.allowed {
        return Ok(DirectHandleResizePlan {
            node_id: node_id.into(),
            handle,
            dx,
            dy,
            target_bounds: target,
            allowed: false,
            blocked: size_plan.blocked.clone(),
            resize_plan: Some(size_plan),
            movement_plan: None,
            note: "handle resize blocked while changing semantic size".into(),
        });
    }

    let mut candidate = scene.clone();
    apply_resize_plan_to_candidate(&mut candidate, &size_plan)?;
    let resized = candidate
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .and_then(query::bounds)
        .ok_or_else(|| anyhow!("node {node_id} became unmeasurable after semantic resize"))?;
    let move_dx = target[0] - resized[0];
    let move_dy = target[1] - resized[1];
    let movement_plan = if move_dx.abs() > 0.0001 || move_dy.abs() > 0.0001 {
        let plan = plan_direct_manipulation(&candidate, node_id, move_dx, move_dy)?;
        if !plan.allowed {
            return Ok(DirectHandleResizePlan {
                node_id: node_id.into(),
                handle,
                dx,
                dy,
                target_bounds: target,
                allowed: false,
                blocked: plan.blocked.clone(),
                resize_plan: Some(size_plan),
                movement_plan: Some(plan),
                note: "handle resize blocked while preserving the opposite-edge anchor".into(),
            });
        }
        apply_movement_plan_to_candidate(&mut candidate, &plan)?;
        Some(plan)
    } else {
        None
    };

    let final_bounds = candidate
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .and_then(query::bounds)
        .ok_or_else(|| anyhow!("node {node_id} became unmeasurable after handle resize"))?;
    let controls_horizontal = handle.moves_left() || handle.moves_right();
    let controls_vertical = handle.moves_top() || handle.moves_bottom();
    let horizontal_mismatch = controls_horizontal
        && ((final_bounds[0] - target[0]).abs() > 0.0001
            || (final_bounds[2] - target[2]).abs() > 0.0001);
    let vertical_mismatch = controls_vertical
        && ((final_bounds[1] - target[1]).abs() > 0.0001
            || (final_bounds[3] - target[3]).abs() > 0.0001);
    if horizontal_mismatch || vertical_mismatch {
        return Ok(DirectHandleResizePlan {
            node_id: node_id.into(),
            handle,
            dx,
            dy,
            target_bounds: target,
            allowed: false,
            resize_plan: Some(size_plan),
            movement_plan,
            blocked: vec![ManipulationBlock {
                axis: if horizontal_mismatch {
                    ManipulationAxis::Horizontal
                } else {
                    ManipulationAxis::Vertical
                },
                constraint_ids: vec![],
                properties: vec!["bounds".into()],
                reason: format!(
                    "candidate handle resize resolved to {:?}, not requested {:?}",
                    final_bounds, target
                ),
            }],
            note: "handle resize failed exact opposite-edge anchor validation".into(),
        });
    }
    let mut resolved_target = target;
    if !controls_horizontal {
        resolved_target[0] = final_bounds[0];
        resolved_target[2] = final_bounds[2];
    }
    if !controls_vertical {
        resolved_target[1] = final_bounds[1];
        resolved_target[3] = final_bounds[3];
    }

    Ok(DirectHandleResizePlan {
        node_id: node_id.into(),
        handle,
        dx,
        dy,
        target_bounds: resolved_target,
        allowed: true,
        resize_plan: Some(size_plan),
        movement_plan,
        blocked: vec![],
        note: "handle resize preserves explicit edge/corner anchor semantics".into(),
    })
}

pub fn apply_direct_handle_resize_source(
    source: &str,
    plan: &DirectHandleResizePlan,
) -> Result<String> {
    if !plan.allowed {
        return Err(anyhow!(
            "direct handle resize of {} is blocked: {}",
            plan.node_id,
            plan.note
        ));
    }
    let mut edited = source.to_string();
    if let Some(resize_plan) = &plan.resize_plan {
        edited = apply_direct_resize_source(&edited, resize_plan)?;
    }
    if let Some(movement_plan) = &plan.movement_plan {
        edited = apply_direct_manipulation_source(&edited, movement_plan)?;
    }
    Ok(edited)
}
