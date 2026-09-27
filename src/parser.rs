use crate::model::*;
use anyhow::{Context, Result, anyhow};
use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Parser)]
#[grammar = "glyph.pest"]
struct GlyphParser;

fn unquote(raw: &str) -> String {
    raw.trim_matches('"').replace("\\\"", "\"")
}
fn line_of(pair: &Pair<Rule>) -> usize {
    pair.as_span().start_pos().line_col().0
}
fn property_ref(raw: &str) -> Result<PropertyRef> {
    let (node, property) = raw
        .split_once('.')
        .ok_or_else(|| anyhow!("invalid property reference {raw}"))?;
    Ok(PropertyRef {
        node: node.to_string(),
        property: property.to_string(),
    })
}
fn parse_quantity(raw: &str) -> Result<Quantity> {
    let (number, unit) = if let Some(v) = raw.strip_suffix("deg") {
        (v, Unit::Deg)
    } else if let Some(v) = raw.strip_suffix("rad") {
        (v, Unit::Rad)
    } else if let Some(v) = raw.strip_suffix("px") {
        (v, Unit::Px)
    } else if let Some(v) = raw.strip_suffix("ms") {
        (v, Unit::Ms)
    } else if let Some(v) = raw.strip_suffix('%') {
        (v, Unit::Percent)
    } else if let Some(v) = raw.strip_suffix('s') {
        (v, Unit::S)
    } else {
        return Err(anyhow!("unsupported unit in {raw}"));
    };
    Ok(Quantity {
        value: number.parse()?,
        unit,
    })
}

pub fn parse_value_literal(raw: &str) -> Result<Value> {
    if let Ok(number) = raw.parse::<f64>() {
        return Ok(Value::Number(number));
    }
    if let Ok(quantity) = parse_quantity(raw) {
        return Ok(Value::Quantity(quantity));
    }
    Ok(Value::Text(raw.to_string()))
}

fn parse_scalar(pair: Pair<Rule>) -> Result<(f64, Option<Unit>)> {
    match pair.as_rule() {
        Rule::number => Ok((pair.as_str().parse()?, None)),
        Rule::quantity => {
            let q = parse_quantity(pair.as_str())?;
            Ok((q.canonical_value(), Some(q.unit)))
        }
        _ => Err(anyhow!("expected numeric scalar, got {:?}", pair.as_rule())),
    }
}

fn validate_property_value(property: &str, value: &Value) -> Result<()> {
    validate_value_for_property(property, value).map_err(|message| anyhow!(message))
}

fn parse_value(pair: Pair<Rule>) -> Result<Value> {
    match pair.as_rule() {
        Rule::number => Ok(Value::Number(pair.as_str().parse()?)),
        Rule::quantity => Ok(Value::Quantity(parse_quantity(pair.as_str())?)),
        Rule::string => Ok(Value::Text(unquote(pair.as_str()))),
        Rule::ident => Ok(Value::Text(pair.as_str().to_string())),
        _ => Err(anyhow!("unsupported value {:?}", pair.as_rule())),
    }
}
fn parse_prop(pair: Pair<Rule>) -> Result<(String, Value)> {
    let mut bits = pair.into_inner();
    let key = bits
        .next()
        .context("missing property name")?
        .as_str()
        .to_string();
    let value = parse_value(bits.next().context("missing property value")?)?;
    validate_property_value(&key, &value)?;
    Ok((key, value))
}
fn parse_optional_number(pair: Pair<Rule>) -> Result<Option<f64>> {
    let raw = pair.as_str();
    if raw == "unknown" {
        return Ok(None);
    }
    let value: f64 = raw.parse()?;
    if !(0.0..=1.0).contains(&value) {
        return Err(anyhow!(
            "confidence value must be within [0, 1], got {value}"
        ));
    }
    Ok(Some(value))
}
fn parse_confidence(pair: Pair<Rule>) -> Result<Confidence> {
    let mut values = pair.into_inner();
    Ok(Confidence {
        semantics: parse_optional_number(values.next().context("missing semantics confidence")?)?,
        geometry: parse_optional_number(values.next().context("missing geometry confidence")?)?,
        depth: parse_optional_number(values.next().context("missing depth confidence")?)?,
    })
}
fn parse_relation(raw: &str) -> Result<Relation> {
    match raw {
        "==" => Ok(Relation::Eq),
        ">=" => Ok(Relation::Ge),
        "<=" => Ok(Relation::Le),
        x => Err(anyhow!("unsupported relation {x}")),
    }
}
fn parse_rhs(pair: Pair<Rule>) -> Result<ConstraintRhs> {
    let inner = pair.into_inner().next().context("empty rhs")?;
    match inner.as_rule() {
        Rule::prop_ref => Ok(ConstraintRhs::Property(property_ref(inner.as_str())?)),
        Rule::number => Ok(ConstraintRhs::Number(inner.as_str().parse()?)),
        Rule::quantity => Ok(ConstraintRhs::Quantity(parse_quantity(inner.as_str())?)),
        _ => Err(anyhow!("bad rhs")),
    }
}
fn parse_offset(pair: Pair<Rule>) -> Result<(f64, Option<Unit>)> {
    let mut p = pair.into_inner();
    let sign = p.next().context("missing offset sign")?.as_str();
    let (n, unit) = parse_scalar(p.next().context("missing offset value")?)?;
    Ok((if sign == "-" { -n } else { n }, unit))
}

fn validate_relation_dimensions(
    lhs: &PropertyRef,
    rhs: &ConstraintRhs,
    offset_unit: Option<Unit>,
) -> Result<()> {
    let lhs_dimension = property_dimension(&lhs.property);
    let rhs_dimension = match rhs {
        ConstraintRhs::Property(r) => property_dimension(&r.property),
        ConstraintRhs::Quantity(q) => Some(q.dimension()),
        ConstraintRhs::Number(_) => None,
    };
    if let (Some(lhs_dimension), Some(rhs_dimension)) = (lhs_dimension, rhs_dimension)
        && lhs_dimension != rhs_dimension
    {
        return Err(anyhow!(
            "dimension mismatch: {}.{} is {lhs_dimension:?}, rhs is {rhs_dimension:?}",
            lhs.node,
            lhs.property
        ));
    }
    if let Some(unit) = offset_unit {
        let Some(lhs_dimension) = lhs_dimension else {
            return Err(anyhow!(
                "cannot apply explicit {} offset to untyped property {}.{}",
                unit.suffix(),
                lhs.node,
                lhs.property
            ));
        };
        if unit.dimension() != lhs_dimension {
            return Err(anyhow!(
                "dimension mismatch: {}.{} is {lhs_dimension:?}, offset is {:?} ({})",
                lhs.node,
                lhs.property,
                unit.dimension(),
                unit.suffix()
            ));
        }
    }
    Ok(())
}

fn parse_ratio(pair: Pair<Rule>, label: &str) -> Result<f64> {
    let (value, unit) = parse_scalar(pair)?;
    if let Some(unit) = unit
        && unit != Unit::Percent
    {
        return Err(anyhow!(
            "{label} expects a unitless ratio or percentage, got {}",
            unit.suffix()
        ));
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(anyhow!("{label} must be within [0, 1], got {value}"));
    }
    Ok(value)
}

fn validate_gradient_stops(name: &str, stops: &[GradientStop]) -> Result<()> {
    if stops.len() < 2 {
        return Err(anyhow!("paint {name} requires at least two stops"));
    }
    if stops.windows(2).any(|pair| pair[0].offset > pair[1].offset) {
        return Err(anyhow!(
            "paint {name} gradient stops must be ordered by nondecreasing offset"
        ));
    }
    Ok(())
}

fn parse_paint(pair: Pair<Rule>) -> Result<(String, Paint)> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("missing paint name")?
        .as_str()
        .to_string();
    let kind = inner
        .next()
        .context("missing paint kind")?
        .as_str()
        .to_string();
    let mut from = (0.0, 0.0);
    let mut to = (1.0, 0.0);
    let mut center = (0.5, 0.5);
    let mut radius = 0.5;
    let mut stops = Vec::new();
    for item in inner {
        match item.as_rule() {
            Rule::paint_from => {
                let mut values = item.into_inner();
                from = (
                    parse_ratio(
                        values.next().context("missing paint from x")?,
                        "paint from x",
                    )?,
                    parse_ratio(
                        values.next().context("missing paint from y")?,
                        "paint from y",
                    )?,
                );
            }
            Rule::paint_to => {
                let mut values = item.into_inner();
                to = (
                    parse_ratio(values.next().context("missing paint to x")?, "paint to x")?,
                    parse_ratio(values.next().context("missing paint to y")?, "paint to y")?,
                );
            }
            Rule::paint_center => {
                let mut values = item.into_inner();
                center = (
                    parse_ratio(
                        values.next().context("missing paint center x")?,
                        "paint center x",
                    )?,
                    parse_ratio(
                        values.next().context("missing paint center y")?,
                        "paint center y",
                    )?,
                );
            }
            Rule::paint_radius => {
                radius = parse_ratio(
                    item.into_inner().next().context("missing paint radius")?,
                    "paint radius",
                )?;
                if radius <= 0.0 {
                    return Err(anyhow!("paint radius must be greater than zero"));
                }
            }
            Rule::paint_stop => {
                let mut values = item.into_inner();
                let offset = parse_ratio(
                    values.next().context("missing paint stop offset")?,
                    "paint stop offset",
                )?;
                let color = unquote(values.next().context("missing paint stop color")?.as_str());
                let opacity = if let Some(value) = values.next() {
                    parse_ratio(value, "paint stop opacity")?
                } else {
                    1.0
                };
                stops.push(GradientStop {
                    offset,
                    color,
                    opacity,
                });
            }
            _ => {}
        }
    }
    validate_gradient_stops(&name, &stops)?;
    let paint = match kind.as_str() {
        "linear" => Paint::Linear {
            x1: from.0,
            y1: from.1,
            x2: to.0,
            y2: to.1,
            stops,
        },
        "radial" => Paint::Radial {
            cx: center.0,
            cy: center.1,
            r: radius,
            stops,
        },
        _ => return Err(anyhow!("unsupported paint kind {kind}")),
    };
    Ok((name, paint))
}

fn parse_style(pair: Pair<Rule>) -> Result<(String, BTreeMap<String, Value>)> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("missing style name")?
        .as_str()
        .to_string();
    let mut props = BTreeMap::new();
    for item in inner {
        if item.as_rule() == Rule::prop {
            let (k, v) = parse_prop(item)?;
            props.insert(k, v);
        }
    }
    Ok((name, props))
}

fn parse_node(
    pair: Pair<Rule>,
    source: &str,
    styles: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<Node> {
    let line = line_of(&pair);
    let mut inner = pair.into_inner();
    let kind = match inner.next().context("missing node kind")?.as_str() {
        "rect" => NodeKind::Rect,
        "ellipse" => NodeKind::Ellipse,
        "path" => NodeKind::Path,
        "text" => NodeKind::Text,
        "image" => NodeKind::Image,
        "group" => NodeKind::Group,
        x => return Err(anyhow!("unsupported node kind {x}")),
    };
    let name = inner
        .next()
        .context("missing node name")?
        .as_str()
        .to_string();
    let mut id = name.clone();
    let mut roles = Vec::new();
    let mut members = Vec::new();
    let mut props = BTreeMap::new();
    let mut style_names = Vec::new();
    let mut confidence = None;
    for item in inner {
        match item.as_rule() {
            Rule::id_attr => {
                id = unquote(
                    item.into_inner()
                        .next()
                        .context("missing id string")?
                        .as_str(),
                );
            }
            Rule::role => roles.extend(
                item.into_inner()
                    .filter(|p| p.as_rule() == Rule::string)
                    .map(|p| unquote(p.as_str())),
            ),
            Rule::style_use => style_names.push(
                item.into_inner()
                    .next()
                    .context("missing style name")?
                    .as_str()
                    .to_string(),
            ),
            Rule::confidence => confidence = Some(parse_confidence(item)?),
            Rule::group_member => {
                members.push(
                    item.into_inner()
                        .next()
                        .context("missing group member")?
                        .as_str()
                        .to_string(),
                );
            }
            Rule::prop => {
                let (k, v) = parse_prop(item)?;
                props.insert(k, v);
            }
            _ => {}
        }
    }
    let explicit = props.clone();
    props.clear();
    for style in style_names {
        let values = styles
            .get(&style)
            .ok_or_else(|| anyhow!("unknown style {style} used by node {id}"))?;
        for (k, v) in values {
            props.insert(k.clone(), v.clone());
        }
        roles.push(format!("style:{style}"));
    }
    for (k, v) in explicit {
        props.insert(k, v);
    }
    if kind != NodeKind::Group && !members.is_empty() {
        return Err(anyhow!("only group nodes may declare member entries"));
    }
    Ok(Node {
        id,
        name,
        kind,
        roles,
        members,
        authored: AuthoredNode { props },
        derived: DerivedNode::default(),
        provenance: Provenance {
            source: source.to_string(),
            line,
            authored: true,
        },
        confidence,
        component_origin: None,
    })
}

fn parse_component_binding(pair: Pair<Rule>) -> Result<ComponentBinding> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("missing binding name")?
        .as_str()
        .to_string();
    let type_name = inner.next().context("missing binding type")?.as_str();
    let value_type = ValueType::from_name(type_name)
        .ok_or_else(|| anyhow!("unknown component value type {type_name}"))?;
    let target = property_ref(inner.next().context("missing binding target")?.as_str())?;
    let target_type = property_value_type(&target.property).ok_or_else(|| {
        anyhow!(
            "component binding {name} targets untyped property {}.{}",
            target.node,
            target.property
        )
    })?;
    if target_type != value_type {
        return Err(anyhow!(
            "component binding {name} declares {value_type:?} but {}.{} is {target_type:?}",
            target.node,
            target.property
        ));
    }
    let default = if let Some(default_pair) = inner.next() {
        let value_pair = default_pair
            .into_inner()
            .next()
            .context("missing binding default value")?;
        let value = parse_value(value_pair)?;
        validate_value_type(value_type, &value)
            .map_err(|message| anyhow!("component binding {name} {message}"))?;
        Some(value)
    } else {
        None
    };
    Ok(ComponentBinding {
        name,
        value_type,
        target,
        default,
    })
}

fn parse_component_variant(pair: Pair<Rule>) -> Result<ComponentVariant> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("missing variant name")?
        .as_str()
        .to_string();
    let mut overrides = Vec::new();
    for item in inner {
        let mut bits = item.into_inner();
        let target = property_ref(bits.next().context("missing variant target")?.as_str())?;
        let value = parse_value(bits.next().context("missing variant value")?)?;
        validate_value_for_property(&target.property, &value)
            .map_err(|message| anyhow!(message))?;
        overrides.push(StateOverride { target, value });
    }
    Ok(ComponentVariant { name, overrides })
}

fn local_node<'a>(nodes: &'a [Node], target: &PropertyRef) -> Result<&'a Node> {
    nodes
        .iter()
        .find(|node| node.id == target.node || node.name == target.node)
        .ok_or_else(|| anyhow!("unknown component-local node {}", target.node))
}

fn local_node_mut<'a>(nodes: &'a mut [Node], target: &PropertyRef) -> Result<&'a mut Node> {
    nodes
        .iter_mut()
        .find(|node| node.id == target.node || node.name == target.node)
        .ok_or_else(|| anyhow!("unknown component-local node {}", target.node))
}

fn validate_component_target(nodes: &[Node], target: &PropertyRef, context: &str) -> Result<()> {
    let node = local_node(nodes, target)?;
    if !node.authored.props.contains_key(&target.property) {
        return Err(anyhow!(
            "{context} targets missing authored property {}.{}",
            target.node,
            target.property
        ));
    }
    Ok(())
}

fn apply_local_value(nodes: &mut [Node], target: &PropertyRef, value: &Value) -> Result<()> {
    validate_value_for_property(&target.property, value).map_err(|message| anyhow!(message))?;
    let node = local_node_mut(nodes, target)?;
    if !node.authored.props.contains_key(&target.property) {
        return Err(anyhow!(
            "override targets missing authored property {}.{}",
            target.node,
            target.property
        ));
    }
    node.authored
        .props
        .insert(target.property.clone(), value.clone());
    Ok(())
}

fn resolve_group_members(nodes: &mut [Node]) -> Result<()> {
    let identities = nodes
        .iter()
        .map(|node| (node.id.clone(), node.name.clone()))
        .collect::<Vec<_>>();
    for index in 0..nodes.len() {
        if nodes[index].kind != NodeKind::Group {
            continue;
        }
        let group_id = nodes[index].id.clone();
        let requested = nodes[index].members.clone();
        let mut resolved = Vec::new();
        for reference in requested {
            let matches = identities
                .iter()
                .filter(|(id, name)| id == &reference || name == &reference)
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>();
            if matches.is_empty() {
                return Err(anyhow!(
                    "group {group_id} references missing member {reference}"
                ));
            }
            if matches.len() > 1 {
                return Err(anyhow!(
                    "group {group_id} member reference {reference} is ambiguous"
                ));
            }
            let member = matches[0].clone();
            if member == group_id {
                return Err(anyhow!("group {group_id} cannot contain itself"));
            }
            if resolved.contains(&member) {
                return Err(anyhow!(
                    "group {group_id} declares duplicate member {member}"
                ));
            }
            resolved.push(member);
        }
        nodes[index].members = resolved;
    }

    let mut owner = BTreeMap::<String, String>::new();
    for group in nodes.iter().filter(|node| node.kind == NodeKind::Group) {
        for member in &group.members {
            if let Some(previous) = owner.insert(member.clone(), group.id.clone()) {
                return Err(anyhow!(
                    "node {member} belongs to multiple groups ({previous}, {})",
                    group.id
                ));
            }
        }
    }

    fn visit(
        id: &str,
        nodes: &[Node],
        visiting: &mut Vec<String>,
        done: &mut BTreeSet<String>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if let Some(position) = visiting.iter().position(|entry| entry == id) {
            let mut cycle = visiting[position..].to_vec();
            cycle.push(id.to_string());
            return Err(anyhow!("group membership cycle: {}", cycle.join(" -> ")));
        }
        let Some(node) = nodes.iter().find(|node| node.id == id) else {
            return Ok(());
        };
        if node.kind != NodeKind::Group {
            done.insert(id.to_string());
            return Ok(());
        }
        visiting.push(id.to_string());
        for member in &node.members {
            visit(member, nodes, visiting, done)?;
        }
        visiting.pop();
        done.insert(id.to_string());
        Ok(())
    }

    let group_ids = nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Group)
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();
    let mut done = BTreeSet::new();
    for group_id in group_ids {
        visit(&group_id, nodes, &mut Vec::new(), &mut done)?;
    }
    Ok(())
}

fn parse_component_definition(
    pair: Pair<Rule>,
    source: &str,
    styles: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<ComponentDefinition> {
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .context("missing component name")?
        .as_str()
        .to_string();
    let mut parameters = Vec::new();
    let mut slots = Vec::new();
    let mut variants = Vec::new();
    let mut nodes = Vec::new();
    for item in inner {
        match item.as_rule() {
            Rule::component_param => parameters.push(parse_component_binding(item)?),
            Rule::component_slot => slots.push(parse_component_binding(item)?),
            Rule::component_variant => variants.push(parse_component_variant(item)?),
            Rule::node => nodes.push(parse_node(item, source, styles)?),
            _ => {}
        }
    }
    resolve_group_members(&mut nodes)?;
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            let a = &nodes[i];
            let b = &nodes[j];
            if a.id == b.id || a.name == b.name || a.id == b.name || a.name == b.id {
                return Err(anyhow!(
                    "component {name} has ambiguous local node identities {} / {}",
                    a.id,
                    b.id
                ));
            }
        }
    }
    for (index, binding) in parameters.iter().chain(slots.iter()).enumerate() {
        if parameters
            .iter()
            .chain(slots.iter())
            .take(index)
            .any(|prior| prior.name == binding.name)
        {
            return Err(anyhow!(
                "component {name} has duplicate interface binding {}",
                binding.name
            ));
        }
        validate_component_target(
            &nodes,
            &binding.target,
            &format!("component binding {}", binding.name),
        )?;
    }
    for (index, variant) in variants.iter().enumerate() {
        if variants[..index]
            .iter()
            .any(|prior| prior.name == variant.name)
        {
            return Err(anyhow!(
                "component {name} has duplicate variant {}",
                variant.name
            ));
        }
        for override_value in &variant.overrides {
            validate_component_target(
                &nodes,
                &override_value.target,
                &format!("variant {}", variant.name),
            )?;
        }
    }
    Ok(ComponentDefinition {
        name,
        parameters,
        slots,
        variants,
        nodes,
    })
}

fn parse_constraint(pair: Pair<Rule>, index: usize) -> Result<Constraint> {
    let mut inner = pair.into_inner();
    let lhs = property_ref(inner.next().context("missing lhs")?.as_str())?;
    let relation = parse_relation(inner.next().context("missing relation")?.as_str())?;
    let rhs = parse_rhs(inner.next().context("missing rhs")?)?;
    let mut offset = 0.0;
    let mut offset_unit = None;
    let mut strength = Strength::Required;
    for item in inner {
        match item.as_rule() {
            Rule::offset => {
                (offset, offset_unit) = parse_offset(item)?;
            }
            Rule::strength => {
                strength = match item.as_str() {
                    "@required" => Strength::Required,
                    "@strong" => Strength::Strong,
                    "@medium" => Strength::Medium,
                    "@weak" => Strength::Weak,
                    _ => Strength::Required,
                }
            }
            _ => {}
        }
    }
    validate_relation_dimensions(&lhs, &rhs, offset_unit)?;
    Ok(Constraint {
        id: format!("constraint.{index}"),
        lhs,
        relation,
        rhs,
        offset,
        offset_unit,
        strength,
        source_order: index,
    })
}
fn parse_assertion(pair: Pair<Rule>, index: usize) -> Result<Assertion> {
    let mut inner = pair.into_inner();
    let lhs = property_ref(inner.next().context("missing assertion lhs")?.as_str())?;
    let relation = parse_relation(inner.next().context("missing relation")?.as_str())?;
    let rhs = parse_rhs(inner.next().context("missing rhs")?)?;
    let mut offset = 0.0;
    let mut offset_unit = None;
    for item in inner {
        if item.as_rule() == Rule::offset {
            (offset, offset_unit) = parse_offset(item)?;
        }
    }
    validate_relation_dimensions(&lhs, &rhs, offset_unit)?;
    Ok(Assertion {
        id: format!("assertion.{index}"),
        lhs,
        relation,
        rhs,
        offset,
        offset_unit,
    })
}
fn parse_animation(pair: Pair<Rule>) -> Result<AnimationSpec> {
    let mut i = pair.into_inner();
    let target = property_ref(i.next().context("missing animation target")?.as_str())?;
    let (from, from_unit) = parse_scalar(i.next().context("missing from")?)?;
    let (to, to_unit) = parse_scalar(i.next().context("missing to")?)?;
    let (duration, duration_unit) = parse_scalar(i.next().context("missing duration")?)?;
    let target_dimension = property_dimension(&target.property);
    for (label, unit) in [("from", from_unit), ("to", to_unit)] {
        if let Some(unit) = unit {
            let Some(expected) = target_dimension else {
                return Err(anyhow!(
                    "animation target {}.{} has no declared dimensional type for explicit {label} unit {}",
                    target.node,
                    target.property,
                    unit.suffix()
                ));
            };
            if unit.dimension() != expected {
                return Err(anyhow!(
                    "animation target {}.{} is {expected:?}, {label} is {:?} ({})",
                    target.node,
                    target.property,
                    unit.dimension(),
                    unit.suffix()
                ));
            }
        }
    }
    if let Some(unit) = duration_unit
        && unit.dimension() != Dimension::Time
    {
        return Err(anyhow!(
            "animation duration expects Time, got {:?} ({})",
            unit.dimension(),
            unit.suffix()
        ));
    }
    if duration < 0.0 {
        return Err(anyhow!("animation duration must be nonnegative"));
    }
    let easing = i
        .next()
        .map(|p| unquote(p.as_str()))
        .unwrap_or_else(|| "linear".into());
    Ok(AnimationSpec {
        target,
        from,
        to,
        duration,
        from_unit,
        to_unit,
        duration_unit,
        easing,
    })
}
fn parse_state(pair: Pair<Rule>) -> Result<SceneState> {
    let mut i = pair.into_inner();
    let name = i.next().context("missing state name")?.as_str().to_string();
    let mut overrides = Vec::new();
    for item in i {
        if item.as_rule() == Rule::state_set {
            let mut x = item.into_inner();
            let target = property_ref(x.next().context("missing state target")?.as_str())?;
            let value = parse_value(x.next().context("missing state value")?)?;
            validate_property_value(&target.property, &value)?;
            overrides.push(StateOverride { target, value });
        }
    }
    Ok(SceneState { name, overrides })
}

fn scale_translate(node: &mut Node, ox: f64, oy: f64, scale: f64) {
    fn number(node: &Node, key: &str, default: f64) -> f64 {
        node.authored
            .props
            .get(key)
            .and_then(Value::number)
            .unwrap_or(default)
    }
    fn set_number(node: &mut Node, key: &str, value: f64) {
        match node.authored.props.get_mut(key) {
            Some(Value::Number(number)) => *number = value,
            Some(Value::Quantity(quantity)) => quantity.value = value,
            Some(Value::Text(_)) => {}
            None => {
                node.authored.props.insert(key.into(), Value::Number(value));
            }
        }
    }

    if node.kind == NodeKind::Path {
        let origin_x = number(node, "origin_x", 0.0);
        let origin_y = number(node, "origin_y", 0.0);
        let local_scale = number(node, "scale", 1.0);
        let local_scale_x = node.authored.props.get("scale_x").and_then(Value::number);
        let local_scale_y = node.authored.props.get("scale_y").and_then(Value::number);
        let local_translate_x = number(node, "translate_x", 0.0);
        let local_translate_y = number(node, "translate_y", 0.0);

        set_number(node, "scale", local_scale * scale);
        if let Some(value) = local_scale_x {
            set_number(node, "scale_x", value * scale);
        }
        if let Some(value) = local_scale_y {
            set_number(node, "scale_y", value * scale);
        }
        set_number(
            node,
            "translate_x",
            ox + local_translate_x * scale + (scale - 1.0) * origin_x,
        );
        set_number(
            node,
            "translate_y",
            oy + local_translate_y * scale + (scale - 1.0) * origin_y,
        );
        if let Some(stroke_width) = node
            .authored
            .props
            .get("stroke_width")
            .and_then(Value::number)
        {
            set_number(node, "stroke_width", stroke_width * scale);
        }
        return;
    }

    for (key, val) in node.authored.props.iter_mut() {
        let transform = |base: f64| match key.as_str() {
            "x" | "cx" => base * scale + ox,
            "y" | "cy" => base * scale + oy,
            _ if property_dimension(key) == Some(Dimension::Length) => base * scale,
            _ => base,
        };
        match val {
            Value::Number(n) => *n = transform(*n),
            Value::Quantity(q) if q.dimension() == Dimension::Length => {
                q.value = transform(q.value)
            }
            Value::Quantity(_) | Value::Text(_) => {}
        }
    }
}
fn instantiate(
    pair: Pair<Rule>,
    source: &str,
    components: &[ComponentDefinition],
    styles: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<Vec<Node>> {
    let line = line_of(&pair);
    let mut i = pair.into_inner();
    let component = i.next().context("missing component")?.as_str().to_string();
    let instance_name = i
        .next()
        .context("missing instance name")?
        .as_str()
        .to_string();
    let mut instance_id = instance_name.clone();
    let mut roles = Vec::new();
    let mut props = BTreeMap::new();
    let mut style_names = Vec::new();
    let mut parameter_values = BTreeMap::new();
    let mut slot_values = BTreeMap::new();
    let mut selected_variant = None;
    let mut overrides = Vec::new();
    for item in i {
        match item.as_rule() {
            Rule::id_attr => {
                instance_id = unquote(item.into_inner().next().context("missing id")?.as_str())
            }
            Rule::role => roles.extend(
                item.into_inner()
                    .filter(|p| p.as_rule() == Rule::string)
                    .map(|p| unquote(p.as_str())),
            ),
            Rule::style_use => style_names.push(
                item.into_inner()
                    .next()
                    .context("missing style")?
                    .as_str()
                    .to_string(),
            ),
            Rule::instance_param | Rule::instance_slot => {
                let is_slot = item.as_rule() == Rule::instance_slot;
                let mut bits = item.into_inner();
                let name = bits
                    .next()
                    .context("missing instance binding name")?
                    .as_str()
                    .to_string();
                let value = parse_value(bits.next().context("missing instance binding value")?)?;
                let values = if is_slot {
                    &mut slot_values
                } else {
                    &mut parameter_values
                };
                if values.insert(name.clone(), value).is_some() {
                    return Err(anyhow!("duplicate instance binding {name}"));
                }
            }
            Rule::instance_variant => {
                let name = item
                    .into_inner()
                    .next()
                    .context("missing instance variant")?
                    .as_str()
                    .to_string();
                if selected_variant.replace(name).is_some() {
                    return Err(anyhow!("an instance may select only one variant"));
                }
            }
            Rule::instance_override => {
                let mut bits = item.into_inner();
                let target =
                    property_ref(bits.next().context("missing override target")?.as_str())?;
                let value = parse_value(bits.next().context("missing override value")?)?;
                overrides.push(StateOverride { target, value });
            }
            Rule::prop => {
                let (k, v) = parse_prop(item)?;
                props.insert(k, v);
            }
            _ => {}
        }
    }
    for st in style_names {
        let vals = styles
            .get(&st)
            .ok_or_else(|| anyhow!("unknown style {st}"))?;
        for (k, v) in vals {
            props.entry(k.clone()).or_insert(v.clone());
        }
        roles.push(format!("style:{st}"));
    }
    let def = components
        .iter()
        .find(|c| c.name == component)
        .ok_or_else(|| anyhow!("unknown component {component}"))?;
    let mut working = def.nodes.clone();

    for binding in def.parameters.iter().chain(def.slots.iter()) {
        if let Some(value) = &binding.default {
            apply_local_value(&mut working, &binding.target, value)?;
        }
    }
    if let Some(name) = &selected_variant {
        let variant = def
            .variants
            .iter()
            .find(|variant| &variant.name == name)
            .ok_or_else(|| anyhow!("unknown variant {name} for component {component}"))?;
        for override_value in &variant.overrides {
            apply_local_value(&mut working, &override_value.target, &override_value.value)?;
        }
        roles.push(format!("variant:{name}"));
    }

    for (kind, bindings, values) in [
        ("parameter", def.parameters.as_slice(), &parameter_values),
        ("slot", def.slots.as_slice(), &slot_values),
    ] {
        for binding in bindings {
            if binding.default.is_none() && !values.contains_key(&binding.name) {
                return Err(anyhow!(
                    "instance {instance_name} is missing required {kind} {}",
                    binding.name
                ));
            }
        }
        for (name, value) in values {
            let binding = bindings
                .iter()
                .find(|binding| &binding.name == name)
                .ok_or_else(|| anyhow!("unknown {kind} {name} for component {component}"))?;
            validate_value_type(binding.value_type, value)
                .map_err(|message| anyhow!("{kind} {name} {message}"))?;
            apply_local_value(&mut working, &binding.target, value)?;
        }
    }
    for override_value in &overrides {
        apply_local_value(&mut working, &override_value.target, &override_value.value)?;
    }

    let ox = props.get("x").and_then(Value::number).unwrap_or(0.0);
    let oy = props.get("y").and_then(Value::number).unwrap_or(0.0);
    let scale = props.get("scale").and_then(Value::number).unwrap_or(1.0);
    let mut out = Vec::new();
    for template in &working {
        let mut n = template.clone();
        n.id = format!("{instance_id}/{}", template.id);
        n.name = format!("{instance_name}/{}", template.name);
        n.members = template
            .members
            .iter()
            .map(|member| format!("{instance_id}/{member}"))
            .collect();
        n.roles.extend([
            "component-instance".into(),
            format!("component:{component}"),
        ]);
        n.roles.extend(roles.clone());
        let mut parameter_bindings: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for binding in def.parameters.iter().filter(|binding| {
            binding.target.node == template.id || binding.target.node == template.name
        }) {
            parameter_bindings
                .entry(binding.target.property.clone())
                .or_default()
                .push(binding.name.clone());
        }
        let overridden_properties = overrides
            .iter()
            .filter(|override_value| {
                override_value.target.node == template.id
                    || override_value.target.node == template.name
            })
            .map(|override_value| override_value.target.property.clone())
            .collect();
        n.component_origin = Some(ComponentInstanceOrigin {
            instance_id: instance_id.clone(),
            instance_name: instance_name.clone(),
            component: component.clone(),
            local_id: template.id.clone(),
            local_name: template.name.clone(),
            offset_x: ox,
            offset_y: oy,
            scale,
            parameter_bindings,
            overridden_properties,
        });
        scale_translate(&mut n, ox, oy, scale);
        n.provenance = Provenance {
            source: source.into(),
            line,
            authored: true,
        };
        out.push(n);
    }
    Ok(out)
}
fn rng01(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 11) as f64) / ((1u64 << 53) as f64)
}
fn expand_repeat(pair: Pair<Rule>, nodes: &[Node]) -> Result<Vec<Node>> {
    let mut i = pair.into_inner();
    let target = i
        .next()
        .context("missing repeat target")?
        .as_str()
        .to_string();
    let count: i64 = i.next().context("missing repeat count")?.as_str().parse()?;
    if count < 0 {
        return Err(anyhow!("repeat count must be nonnegative"));
    }
    let mut seed = 0u64;
    let mut props = BTreeMap::new();
    let mut first = i.next();
    if let Some(p) = first.take() {
        if p.as_rule() == Rule::number {
            seed = p.as_str().parse::<u64>()?;
        } else if p.as_rule() == Rule::prop {
            let (k, v) = parse_prop(p)?;
            props.insert(k, v);
        }
    }
    for p in i {
        if p.as_rule() == Rule::prop {
            let (k, v) = parse_prop(p)?;
            props.insert(k, v);
        }
    }
    let proto = nodes
        .iter()
        .find(|n| n.id == target || n.name == target)
        .ok_or_else(|| anyhow!("repeat target {target} not found"))?;
    let dx = props.get("dx").and_then(Value::number).unwrap_or(0.0);
    let dy = props.get("dy").and_then(Value::number).unwrap_or(0.0);
    let jx = props.get("jitter_x").and_then(Value::number).unwrap_or(0.0);
    let jy = props.get("jitter_y").and_then(Value::number).unwrap_or(0.0);
    let scale_step = props
        .get("scale_step")
        .and_then(Value::number)
        .unwrap_or(0.0);
    let mut out = Vec::new();
    let mut state = seed;
    for k in 1..=count {
        let mut n = proto.clone();
        n.id = format!("{}~{}", proto.id, k);
        n.name = format!("{}~{}", proto.name, k);
        n.roles
            .extend(["generated".into(), format!("repeat:{}", proto.id)]);
        let ox = dx * k as f64 + (rng01(&mut state) * 2.0 - 1.0) * jx;
        let oy = dy * k as f64 + (rng01(&mut state) * 2.0 - 1.0) * jy;
        let sc = (1.0 + scale_step * k as f64).max(0.0);
        scale_translate(&mut n, ox, oy, sc);
        n.provenance.authored = false;
        out.push(n);
    }
    Ok(out)
}

pub fn validate_source_syntax(input: &str) -> Result<()> {
    GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    Ok(())
}

pub fn parse_import_specs(input: &str) -> Result<Vec<ImportSpec>> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let mut imports = Vec::new();
    for item in file.into_inner() {
        if item.as_rule() != Rule::import_decl {
            continue;
        }
        let mut bits = item.into_inner();
        let path = unquote(bits.next().context("missing import path")?.as_str());
        let alias = bits
            .next()
            .context("missing import alias")?
            .as_str()
            .to_string();
        if alias.contains('/') {
            return Err(anyhow!("import alias {alias} must not contain '/'"));
        }
        if imports
            .iter()
            .any(|existing: &ImportSpec| existing.alias == alias)
        {
            return Err(anyhow!("duplicate import alias {alias}"));
        }
        imports.push(ImportSpec { path, alias });
    }
    Ok(imports)
}

pub(crate) fn direct_authored_node_records(
    input: &str,
) -> Result<Vec<(String, String, String, usize)>> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene_pair = file
        .into_inner()
        .find(|item| item.as_rule() == Rule::scene)
        .context("missing scene")?;
    let mut records = Vec::new();
    for item in scene_pair.into_inner() {
        if item.as_rule() != Rule::node {
            continue;
        }
        let line = line_of(&item);
        let mut bits = item.into_inner();
        let kind = bits
            .next()
            .context("missing node kind")?
            .as_str()
            .to_string();
        let name = bits
            .next()
            .context("missing node name")?
            .as_str()
            .to_string();
        let mut id = name.clone();
        if let Some(next) = bits.next()
            && next.as_rule() == Rule::id_attr
        {
            id = unquote(
                next.into_inner()
                    .next()
                    .context("missing node id")?
                    .as_str(),
            );
        }
        records.push((id, name, kind, line));
    }
    Ok(records)
}

pub fn direct_authored_node_identity(
    input: &str,
    node_id: &str,
) -> Result<(String, String, usize)> {
    for (id, name, _kind, line) in direct_authored_node_records(input)? {
        if id == node_id || name == node_id {
            return Ok((id, name, line));
        }
    }
    Err(anyhow!("node {node_id} not found or not directly authored"))
}

pub fn direct_authored_instance_identity(
    input: &str,
    instance_id: &str,
) -> Result<(String, String, usize)> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene_pair = file
        .into_inner()
        .find(|item| item.as_rule() == Rule::scene)
        .context("missing scene")?;
    for item in scene_pair.into_inner() {
        if item.as_rule() != Rule::instance {
            continue;
        }
        let line = line_of(&item);
        let mut bits = item.into_inner();
        let _component = bits.next().context("missing instance component")?;
        let name = bits
            .next()
            .context("missing instance name")?
            .as_str()
            .to_string();
        let mut id = name.clone();
        if let Some(next) = bits.next()
            && next.as_rule() == Rule::id_attr
        {
            id = unquote(
                next.into_inner()
                    .next()
                    .context("missing instance id")?
                    .as_str(),
            );
        }
        if id == instance_id || name == instance_id {
            return Ok((id, name, line));
        }
    }
    Err(anyhow!(
        "instance {instance_id} not found or not directly authored"
    ))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DirectConstraintSourceSpan {
    pub full: (usize, usize),
    pub relation: (usize, usize),
    pub offset: Option<(usize, usize)>,
    pub strength: (usize, usize),
}

pub(crate) fn direct_authored_constraint_source_span(
    input: &str,
    source_order: usize,
) -> Result<DirectConstraintSourceSpan> {
    if source_order == 0 {
        return Err(anyhow!("constraint source order must be >= 1"));
    }
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene_pair = file
        .into_inner()
        .find(|item| item.as_rule() == Rule::scene)
        .context("missing scene")?;
    let mut ordinal = 0usize;
    for item in scene_pair.into_inner() {
        if item.as_rule() != Rule::constraint {
            continue;
        }
        ordinal += 1;
        if ordinal != source_order {
            continue;
        }
        let item_span = item.as_span();
        let mut relation = None;
        let mut offset = None;
        let mut strength = None;
        for bit in item.into_inner() {
            let span = bit.as_span();
            match bit.as_rule() {
                Rule::relation => relation = Some((span.start(), span.end())),
                Rule::offset => offset = Some((span.start(), span.end())),
                Rule::strength => strength = Some((span.start(), span.end())),
                _ => {}
            }
        }
        return Ok(DirectConstraintSourceSpan {
            full: (item_span.start(), item_span.end()),
            relation: relation.context("constraint missing relation")?,
            offset,
            strength: strength.context("constraint missing strength")?,
        });
    }
    Err(anyhow!(
        "constraint.{source_order} not found in directly authored source"
    ))
}

pub(crate) fn direct_authored_scene_source_span(input: &str) -> Result<(usize, usize)> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene = file
        .into_inner()
        .find(|item| item.as_rule() == Rule::scene)
        .context("missing scene")?;
    let span = scene.as_span();
    Ok((span.start(), span.end()))
}

fn validate_scene_references(nodes: &[Node], paints: &BTreeMap<String, Paint>) -> Result<()> {
    let mut by_id = BTreeMap::new();
    for node in nodes {
        if by_id.insert(node.id.as_str(), node).is_some() {
            return Err(anyhow!("duplicate stable node id {}", node.id));
        }
    }
    for node in nodes {
        for (property, allowed) in [
            (
                "clip",
                &[NodeKind::Rect, NodeKind::Ellipse, NodeKind::Path][..],
            ),
            (
                "mask",
                &[NodeKind::Rect, NodeKind::Ellipse, NodeKind::Path][..],
            ),
        ] {
            let Some(target_id) = node.authored.props.get(property).and_then(Value::text) else {
                continue;
            };
            if target_id == node.id {
                return Err(anyhow!("node {} cannot {property} itself", node.id));
            }
            let target = by_id.get(target_id).ok_or_else(|| {
                anyhow!(
                    "node {} references missing {property} target {target_id}",
                    node.id
                )
            })?;
            if !allowed.contains(&target.kind) {
                return Err(anyhow!(
                    "node {} {property} target {target_id} has unsupported kind {:?}",
                    node.id,
                    target.kind
                ));
            }
            if target.authored.props.contains_key("clip")
                || target.authored.props.contains_key("mask")
            {
                return Err(anyhow!(
                    "{property} definition target {target_id} may not itself use clip or mask in source v1"
                ));
            }
        }
        for property in ["fill_paint", "stroke_paint"] {
            let Some(name) = node.authored.props.get(property).and_then(Value::text) else {
                continue;
            };
            if !paints.contains_key(name) {
                return Err(anyhow!(
                    "node {} references missing {property} {name}",
                    node.id
                ));
            }
        }
    }
    Ok(())
}

pub fn parse_scene(input: &str, source: &str) -> Result<Scene> {
    let imports = parse_import_specs(input)?;
    if !imports.is_empty() {
        return Err(anyhow!(
            "source contains imports; use the project loader so module paths and project boundaries are resolved"
        ));
    }
    parse_scene_with_environment(
        input,
        source,
        BTreeMap::new(),
        BTreeMap::new(),
        Vec::new(),
        Vec::new(),
    )
}

pub(crate) fn parse_scene_with_environment(
    input: &str,
    source: &str,
    seeded_styles: BTreeMap<String, BTreeMap<String, Value>>,
    seeded_paints: BTreeMap<String, Paint>,
    seeded_components: Vec<ComponentDefinition>,
    imports: Vec<ImportSpec>,
) -> Result<Scene> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene_pair = file
        .into_inner()
        .find(|item| item.as_rule() == Rule::scene)
        .context("missing scene")?;
    let mut inner = scene_pair.into_inner();
    let name = unquote(inner.next().context("missing scene name")?.as_str());
    let mut id = name.to_lowercase().replace(' ', "-");
    let mut source_version = CURRENT_SOURCE_VERSION;
    let mut seen_scene_id = false;
    let mut seen_source_version = false;
    let mut canvas = Canvas {
        width: 1024.0,
        height: 1024.0,
        width_unit: None,
        height_unit: None,
        background: None,
    };
    let mut nodes = Vec::new();
    let mut constraints = Vec::new();
    let mut assertions = Vec::new();
    let mut styles = seeded_styles;
    let mut paints = seeded_paints;
    let mut components = seeded_components;
    let mut animations = Vec::new();
    let mut states = Vec::new();
    let mut metadata = BTreeMap::new();
    for item in inner {
        match item.as_rule() {
            Rule::id_attr => {
                if seen_scene_id {
                    return Err(anyhow!("duplicate scene @id attribute"));
                }
                seen_scene_id = true;
                id = unquote(
                    item.into_inner()
                        .next()
                        .context("missing scene id")?
                        .as_str(),
                )
            }
            Rule::version_attr => {
                if seen_source_version {
                    return Err(anyhow!("duplicate scene @version attribute"));
                }
                seen_source_version = true;
                source_version = item
                    .into_inner()
                    .next()
                    .context("missing source version")?
                    .as_str()
                    .parse()?;
                if source_version == 0 || source_version > CURRENT_SOURCE_VERSION {
                    return Err(anyhow!(
                        "unsupported Glyph source version {source_version}; current version is {CURRENT_SOURCE_VERSION}"
                    ));
                }
            }
            Rule::canvas => {
                let mut b = item.into_inner();
                let (width, width_unit) = parse_scalar(b.next().context("missing width")?)?;
                let (height, height_unit) = parse_scalar(b.next().context("missing height")?)?;
                for (axis, unit) in [("width", width_unit), ("height", height_unit)] {
                    if let Some(unit) = unit
                        && unit.dimension() != Dimension::Length
                    {
                        return Err(anyhow!(
                            "canvas {axis} expects Length, got {:?} ({})",
                            unit.dimension(),
                            unit.suffix()
                        ));
                    }
                }
                canvas.width = width;
                canvas.height = height;
                canvas.width_unit = width_unit;
                canvas.height_unit = height_unit;
                if let Some(bg) = b.next() {
                    canvas.background = Some(unquote(bg.as_str()));
                }
            }
            Rule::metadata => {
                let mut b = item.into_inner();
                let key = unquote(b.next().context("missing metadata key")?.as_str());
                let value = unquote(b.next().context("missing metadata value")?.as_str());
                metadata.insert(key, value);
            }
            Rule::style_def => {
                let (n, p) = parse_style(item)?;
                if styles.contains_key(&n) {
                    return Err(anyhow!("duplicate style {n}"));
                }
                styles.insert(n, p);
            }
            Rule::paint_def => {
                let (name, paint) = parse_paint(item)?;
                if paints.insert(name.clone(), paint).is_some() {
                    return Err(anyhow!("duplicate paint {name}"));
                }
            }
            Rule::component_def => {
                let component = parse_component_definition(item, source, &styles)?;
                if components
                    .iter()
                    .any(|existing: &ComponentDefinition| existing.name == component.name)
                {
                    return Err(anyhow!("duplicate component {}", component.name));
                }
                components.push(component);
            }
            Rule::node => nodes.push(parse_node(item, source, &styles)?),
            Rule::instance => nodes.extend(instantiate(item, source, &components, &styles)?),
            Rule::repeat => {
                let generated = expand_repeat(item, &nodes)?;
                nodes.extend(generated);
            }
            Rule::constraint => constraints.push(parse_constraint(item, constraints.len() + 1)?),
            Rule::assertion => assertions.push(parse_assertion(item, assertions.len() + 1)?),
            Rule::animation => animations.push(parse_animation(item)?),
            Rule::state_def => states.push(parse_state(item)?),
            _ => {}
        }
    }
    resolve_group_members(&mut nodes)?;
    validate_scene_references(&nodes, &paints)?;
    metadata
        .entry("engine".into())
        .or_insert_with(|| "Glyph Core".into());
    metadata
        .entry("source".into())
        .or_insert_with(|| source.into());
    metadata
        .entry("sourceModel".into())
        .or_insert_with(|| "semantic-v2".into());
    Ok(Scene {
        schema_version: CURRENT_GSIR_SCHEMA_VERSION,
        source_version,
        revision: 1,
        imports,
        id,
        name,
        canvas,
        nodes,
        constraints,
        assertions,
        styles,
        paints,
        components,
        animations,
        states,
        metadata,
    })
}
