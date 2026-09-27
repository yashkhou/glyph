use crate::{
    language_service::authored_node_reference_count,
    model::{
        ConstraintRhs, PropertyRef, Relation, Strength, Unit, Value, validate_value_for_property,
    },
    parser::{
        direct_authored_component_span_records, direct_authored_constraint_source_span,
        direct_authored_instance_source_span, direct_authored_instance_span_records,
        direct_authored_node_identity, direct_authored_node_records,
        direct_authored_node_source_span, direct_authored_node_span_records,
        direct_authored_scene_source_span, parse_import_specs, parse_scene, validate_source_syntax,
    },
};
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::BTreeSet;

fn skip_string(bytes: &[u8], mut i: usize) -> usize {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return i + 1;
        }
        i += 1;
    }
    bytes.len()
}
fn skip_line_comment(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}
fn find_matching_brace(source: &str, open: usize) -> Result<usize> {
    let b = source.as_bytes();
    let mut i = open + 1;
    let mut depth = 1i32;
    while i < b.len() {
        if b[i] == b'"' {
            i = skip_string(b, i);
            continue;
        }
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'/' {
            i = skip_line_comment(b, i + 2);
            continue;
        }
        match b[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    Err(anyhow!("unclosed block"))
}
fn boundary(b: u8) -> bool {
    !(b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'/')
}
fn token_at(source: &str, start: usize, token: &str) -> bool {
    let b = source.as_bytes();
    let t = token.as_bytes();
    if start + t.len() > b.len() || &b[start..start + t.len()] != t {
        return false;
    }
    let left = start == 0 || boundary(b[start - 1]);
    let right = start + t.len() == b.len() || boundary(b[start + t.len()]);
    left && right
}
fn block_from_source_span(
    source: &str,
    span_start: usize,
    span_end: usize,
    label: &str,
) -> Result<(usize, usize, usize)> {
    let line_start = source[..span_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let start = if source[line_start..span_start].trim().is_empty() {
        line_start
    } else {
        span_start
    };
    let bytes = source.as_bytes();
    let mut cursor = span_start;
    while cursor < span_end {
        if bytes[cursor] == b'"' {
            cursor = skip_string(bytes, cursor);
            continue;
        }
        if cursor + 1 < span_end && bytes[cursor] == b'/' && bytes[cursor + 1] == b'/' {
            cursor = skip_line_comment(bytes, cursor + 2);
            continue;
        }
        if bytes[cursor] == b'{' {
            let close = find_matching_brace(source, cursor)?;
            if close >= span_end {
                return Err(anyhow!(
                    "source span for {label} has mismatched block bounds"
                ));
            }
            return Ok((start, cursor, close));
        }
        cursor += 1;
    }
    Err(anyhow!("source block for {label} not found"))
}

fn node_block(source: &str, node_id: &str) -> Result<(usize, usize, usize)> {
    let (span_start, span_end) = direct_authored_node_source_span(source, node_id)?;
    block_from_source_span(source, span_start, span_end, node_id)
}

fn instance_block(source: &str, instance_id: &str) -> Result<(usize, usize, usize)> {
    let (span_start, span_end) = direct_authored_instance_source_span(source, instance_id)?;
    block_from_source_span(source, span_start, span_end, instance_id)
}

#[derive(Debug, Clone)]
struct DirectLayerOwner {
    id: String,
    name: String,
    kind: String,
    line: usize,
    is_instance: bool,
    span_start: usize,
    span_end: usize,
}

fn direct_layer_owners(source: &str) -> Result<Vec<DirectLayerOwner>> {
    let mut owners = direct_authored_node_span_records(source)?
        .into_iter()
        .map(
            |(id, name, kind, line, span_start, span_end)| DirectLayerOwner {
                id,
                name,
                kind,
                line,
                is_instance: false,
                span_start,
                span_end,
            },
        )
        .collect::<Vec<_>>();
    owners.extend(
        direct_authored_instance_span_records(source)?
            .into_iter()
            .map(
                |(id, name, component, line, span_start, span_end)| DirectLayerOwner {
                    id,
                    name,
                    kind: format!("instance:{component}"),
                    line,
                    is_instance: true,
                    span_start,
                    span_end,
                },
            ),
    );
    owners.sort_by_key(|owner| owner.span_start);
    Ok(owners)
}

fn resolve_layer_owner(source: &str, requested: &str) -> Result<DirectLayerOwner> {
    let owners = direct_layer_owners(source)?;
    let exact = owners
        .iter()
        .filter(|owner| owner.id == requested || owner.name == requested)
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() == 1 {
        return Ok(exact[0].clone());
    }
    if exact.len() > 1 {
        return Err(anyhow!("layer owner {requested} is ambiguous"));
    }

    let matches = owners
        .iter()
        .filter(|owner| {
            owner.is_instance
                && (requested.strip_prefix(&format!("{}/", owner.id)).is_some()
                    || requested
                        .strip_prefix(&format!("{}/", owner.name))
                        .is_some())
        })
        .cloned()
        .collect::<Vec<_>>();
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    Err(anyhow!(
        "node {requested} is not one unambiguous directly-authored layer owner or component-instance child"
    ))
}

fn layer_owner_span(source: &str, owner: &DirectLayerOwner) -> Result<(usize, usize)> {
    let (start, _, close) =
        block_from_source_span(source, owner.span_start, owner.span_end, &owner.id)?;
    let bytes = source.as_bytes();
    let mut end = close + 1;
    while end < bytes.len() && matches!(bytes[end], b' ' | b'\t' | b'\r') {
        end += 1;
    }
    if end + 1 < bytes.len() && bytes[end] == b'/' && bytes[end + 1] == b'/' {
        end = skip_line_comment(bytes, end + 2);
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    Ok((start, end))
}

fn encode_value(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            let mut s = format!("{n}");
            if s.ends_with(".0") {
                s.truncate(s.len() - 2)
            }
            s
        }
        Value::Quantity(q) => format!("{}{}", q.value, q.unit.suffix()),
        Value::Text(t) => format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}

pub fn set_source_property(
    source: &str,
    node_id: &str,
    property: &str,
    value: Value,
) -> Result<String> {
    validate_value_for_property(property, &value).map_err(|message| anyhow!(message))?;
    let (block_start, open, close) = node_block(source, node_id)?;
    let body = &source[open + 1..close];
    let mut i = 0usize;
    let b = body.as_bytes();
    let replacement = encode_value(&value);
    while i < b.len() {
        if b[i] == b'"' {
            i = skip_string(b, i);
            continue;
        }
        if i + 1 < b.len() && b[i] == b'/' && b[i + 1] == b'/' {
            i = skip_line_comment(b, i + 2);
            continue;
        }
        if token_at(body, i, property) {
            let mut j = i + property.len();
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let value_start = j;
            if j >= b.len() {
                break;
            }
            if b[j] == b'"' {
                j = skip_string(b, j);
            } else {
                while j < b.len() && b[j] != b';' && !b[j].is_ascii_whitespace() {
                    j += 1;
                }
            }
            let abs_start = open + 1 + value_start;
            let abs_end = open + 1 + j;
            let mut out = String::with_capacity(source.len() + replacement.len());
            out.push_str(&source[..abs_start]);
            out.push_str(&replacement);
            out.push_str(&source[abs_end..]);
            validate_source_syntax(&out)?;
            return Ok(out);
        }
        i += 1;
    }
    // Insert before the node closing brace using only the header line's leading
    // whitespace. Inline node content must never become indentation.
    let header_line = &source[block_start..open];
    let leading_len = header_line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let header_indent = &header_line[..leading_len];
    let property_indent = format!("{}  ", header_indent);
    let insertion = format!(
        "\n{}{} {};\n{}",
        property_indent, property, replacement, header_indent
    );
    let mut out = String::new();
    out.push_str(&source[..close]);
    out.push_str(&insertion);
    out.push_str(&source[close..]);
    validate_source_syntax(&out)?;
    Ok(out)
}

fn set_source_instance_binding(
    source: &str,
    instance_id: &str,
    keyword: &str,
    binding: &str,
    value: Value,
) -> Result<String> {
    let (block_start, open, close) = instance_block(source, instance_id)?;
    let body = &source[open + 1..close];
    let bytes = body.as_bytes();
    let replacement = encode_value(&value);
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i = skip_line_comment(bytes, i + 2);
            continue;
        }
        if token_at(body, i, keyword) {
            let mut j = i + keyword.len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if token_at(body, j, binding) {
                j += binding.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                let value_start = j;
                if j < bytes.len() && bytes[j] == b'"' {
                    j = skip_string(bytes, j);
                } else {
                    while j < bytes.len() && bytes[j] != b';' && !bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                }
                let abs_start = open + 1 + value_start;
                let abs_end = open + 1 + j;
                let mut out = String::with_capacity(source.len() + replacement.len());
                out.push_str(&source[..abs_start]);
                out.push_str(&replacement);
                out.push_str(&source[abs_end..]);
                validate_source_syntax(&out)?;
                return Ok(out);
            }
        }
        i += 1;
    }
    let header_line = &source[block_start..open];
    let leading_len = header_line
        .chars()
        .take_while(|c| c.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    let header_indent = &header_line[..leading_len];
    let item_indent = format!("{}  ", header_indent);
    let insertion = format!("\n{item_indent}{keyword} {binding} {replacement};\n{header_indent}");
    let mut out = String::with_capacity(source.len() + insertion.len());
    out.push_str(&source[..close]);
    out.push_str(&insertion);
    out.push_str(&source[close..]);
    validate_source_syntax(&out)?;
    Ok(out)
}

pub fn set_source_instance_parameter(
    source: &str,
    instance_id: &str,
    parameter: &str,
    value: Value,
) -> Result<String> {
    set_source_instance_binding(source, instance_id, "param", parameter, value)
}

pub fn set_source_instance_slot(
    source: &str,
    instance_id: &str,
    slot: &str,
    value: Value,
) -> Result<String> {
    set_source_instance_binding(source, instance_id, "slot", slot, value)
}

fn encode_scalar_number(value: f64) -> String {
    let mut encoded = format!("{value}");
    if encoded.ends_with(".0") {
        encoded.truncate(encoded.len() - 2);
    }
    encoded
}

fn source_value_for_unit(value: f64, unit: Option<Unit>) -> f64 {
    match unit {
        Some(Unit::Rad) => value.to_radians(),
        Some(Unit::Percent) => value * 100.0,
        Some(Unit::Ms) => value * 1000.0,
        Some(Unit::Px | Unit::Deg | Unit::S) | None => value,
    }
}

pub fn set_source_constraint_offset(
    source: &str,
    source_order: usize,
    offset: f64,
    unit: Option<Unit>,
) -> Result<String> {
    if !offset.is_finite() {
        return Err(anyhow!("constraint offset must be finite"));
    }
    let span = direct_authored_constraint_source_span(source, source_order)?;
    let mut out = source.to_string();
    if offset.abs() <= f64::EPSILON {
        if let Some((start, end)) = span.offset {
            out.replace_range(start..end, "");
        }
    } else {
        let sign = if offset < 0.0 { "-" } else { "+" };
        let suffix = unit.map(Unit::suffix).unwrap_or("");
        let authored = source_value_for_unit(offset.abs(), unit);
        let replacement = format!("{sign} {}{suffix}", encode_scalar_number(authored));
        if let Some((start, end)) = span.offset {
            out.replace_range(start..end, &replacement);
        } else {
            out.insert_str(span.strength.0, &format!("{replacement} "));
        }
    }
    parse_scene(&out, "<constraint-offset-edit>")?;
    Ok(out)
}

pub fn set_source_constraint_relation(
    source: &str,
    source_order: usize,
    relation: Relation,
) -> Result<String> {
    let span = direct_authored_constraint_source_span(source, source_order)?;
    let replacement = match relation {
        Relation::Eq => "==",
        Relation::Ge => ">=",
        Relation::Le => "<=",
    };
    let mut out = source.to_string();
    out.replace_range(span.relation.0..span.relation.1, replacement);
    parse_scene(&out, "<constraint-relation-edit>")?;
    Ok(out)
}

pub fn set_source_constraint_strength(
    source: &str,
    source_order: usize,
    strength: Strength,
) -> Result<String> {
    let span = direct_authored_constraint_source_span(source, source_order)?;
    let replacement = match strength {
        Strength::Required => "@required",
        Strength::Strong => "@strong",
        Strength::Medium => "@medium",
        Strength::Weak => "@weak",
    };
    let mut out = source.to_string();
    out.replace_range(span.strength.0..span.strength.1, replacement);
    parse_scene(&out, "<constraint-strength-edit>")?;
    Ok(out)
}

pub fn remove_source_constraint(source: &str, source_order: usize) -> Result<String> {
    let span = direct_authored_constraint_source_span(source, source_order)?;
    let line_start = source[..span.full.0]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = source[span.full.1..]
        .find('\n')
        .map(|index| span.full.1 + index + 1)
        .unwrap_or(span.full.1);
    let remove_full_line = source[line_start..span.full.0].trim().is_empty()
        && source[span.full.1..line_end].trim().is_empty();
    let mut out = source.to_string();
    if remove_full_line {
        out.replace_range(line_start..line_end, "");
    } else {
        out.replace_range(span.full.0..span.full.1, "");
    }
    parse_scene(&out, "<constraint-remove>")?;
    Ok(out)
}

fn encode_property_ref(reference: &PropertyRef) -> String {
    format!("{}.{}", reference.node, reference.property)
}

fn encode_constraint_rhs(rhs: &ConstraintRhs) -> String {
    match rhs {
        ConstraintRhs::Property(reference) => encode_property_ref(reference),
        ConstraintRhs::Number(value) => encode_scalar_number(*value),
        ConstraintRhs::Quantity(quantity) => {
            format!(
                "{}{}",
                encode_scalar_number(quantity.value),
                quantity.unit.suffix()
            )
        }
    }
}

fn encode_relation(relation: Relation) -> &'static str {
    match relation {
        Relation::Eq => "==",
        Relation::Ge => ">=",
        Relation::Le => "<=",
    }
}

fn encode_strength(strength: Strength) -> &'static str {
    match strength {
        Strength::Required => "@required",
        Strength::Strong => "@strong",
        Strength::Medium => "@medium",
        Strength::Weak => "@weak",
    }
}

pub fn add_source_constraint(
    source: &str,
    lhs: &PropertyRef,
    relation: Relation,
    rhs: &ConstraintRhs,
    offset: f64,
    offset_unit: Option<Unit>,
    strength: Strength,
) -> Result<String> {
    if !offset.is_finite() {
        return Err(anyhow!("constraint offset must be finite"));
    }
    let (_, scene_end) = direct_authored_scene_source_span(source)?;
    let close = scene_end
        .checked_sub(1)
        .ok_or_else(|| anyhow!("invalid scene source span"))?;
    if source.as_bytes().get(close) != Some(&b'}') {
        return Err(anyhow!("scene source span does not end at closing brace"));
    }
    let mut line = format!(
        "constrain {} {} {}",
        encode_property_ref(lhs),
        encode_relation(relation),
        encode_constraint_rhs(rhs)
    );

    if offset.abs() > f64::EPSILON {
        let sign = if offset < 0.0 { "-" } else { "+" };
        let suffix = offset_unit.map(Unit::suffix).unwrap_or("");
        let authored = source_value_for_unit(offset.abs(), offset_unit);
        line.push_str(&format!(
            " {sign} {}{suffix}",
            encode_scalar_number(authored)
        ));
    }
    line.push(' ');
    line.push_str(encode_strength(strength));
    line.push(';');

    let line_start = source[..close]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let closing_indent = &source[line_start..close];
    let mut out = source.to_string();
    if closing_indent.trim().is_empty() {
        let item_indent = format!("{closing_indent}  ");
        out.insert_str(line_start, &format!("{item_indent}{line}\n"));
    } else {
        out.insert_str(close, &format!("\n  {line}\n"));
    }
    parse_scene(&out, "<constraint-add>")?;
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePlacement {
    Before,
    After,
}

impl std::str::FromStr for NodePlacement {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "before" => Ok(Self::Before),
            "after" => Ok(Self::After),
            _ => Err(format!(
                "unknown node placement {value:?}; expected before or after"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DirectSourceNode {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub parent: Option<String>,
    pub members: Vec<String>,
}

pub fn direct_source_nodes(source: &str) -> Result<Vec<DirectSourceNode>> {
    let owners = direct_layer_owners(source)?;
    let mut nodes = Vec::with_capacity(owners.len());
    for (index, owner) in owners.into_iter().enumerate() {
        let (parent, members) = if owner.is_instance {
            (None, Vec::new())
        } else {
            let parents = group_owners_for_direct_node(source, &owner.id)?;
            if parents.len() > 1 {
                return Err(anyhow!(
                    "node {} belongs to multiple structural groups: {}",
                    owner.id,
                    parents.join(", ")
                ));
            }
            let members = if owner.kind == "group" {
                group_member_spans(source, &owner.id)?
                    .into_iter()
                    .map(|(member, _, _)| {
                        direct_node_reference(source, &member)
                            .map(|(stable_id, _, _)| stable_id)
                            .unwrap_or(member)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            (parents.into_iter().next(), members)
        };
        nodes.push(DirectSourceNode {
            index,
            id: owner.id,
            name: owner.name,
            kind: owner.kind,
            line: owner.line,
            parent,
            members,
        });
    }
    Ok(nodes)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ComponentBindingSummary {
    pub name: String,
    pub value_type: crate::model::ValueType,
    pub target: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ComponentCatalogEntry {
    pub name: String,
    pub parameters: Vec<ComponentBindingSummary>,
    pub slots: Vec<ComponentBindingSummary>,
    pub variants: Vec<String>,
    pub node_count: usize,
}

pub fn component_catalog(scene: &crate::model::Scene) -> Vec<ComponentCatalogEntry> {
    let summarize = |binding: &crate::model::ComponentBinding| ComponentBindingSummary {
        name: binding.name.clone(),
        value_type: binding.value_type,
        target: format!("{}.{}", binding.target.node, binding.target.property),
        required: binding.default.is_none(),
    };
    let mut entries = scene
        .components
        .iter()
        .map(|component| ComponentCatalogEntry {
            name: component.name.clone(),
            parameters: component.parameters.iter().map(summarize).collect(),
            slots: component.slots.iter().map(summarize).collect(),
            variants: component
                .variants
                .iter()
                .map(|variant| variant.name.clone())
                .collect(),
            node_count: component.nodes.len(),
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn valid_source_ident(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/'))
}

fn authored_property_target(block: &str, property: &str) -> Option<String> {
    let bytes = block.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i = skip_line_comment(bytes, i + 2);
            continue;
        }
        if !token_at(block, i, property) {
            i += 1;
            continue;
        }
        let mut cursor = i + property.len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            return None;
        }
        if bytes[cursor] == b'"' {
            let end = skip_string(bytes, cursor);
            if end <= cursor + 1 || end > bytes.len() {
                return None;
            }
            return Some(block[cursor + 1..end - 1].replace("\\\"", "\""));
        }
        let start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b';'
        {
            cursor += 1;
        }
        return (cursor > start).then(|| block[start..cursor].to_string());
    }
    None
}

fn reindent_block(block: &str, indent: &str) -> String {
    let first = block.lines().next().unwrap_or("");
    let source_indent = first
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    block
        .trim_end_matches(['\n', '\r'])
        .lines()
        .map(|line| {
            let local = line.strip_prefix(&source_indent).unwrap_or(line);
            format!("{indent}{local}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn extract_source_component(
    source: &str,
    component_name: &str,
    instance_name: &str,
    stable_id: Option<&str>,
    members: &[String],
) -> Result<(String, String)> {
    if !valid_source_ident(component_name) || component_name.contains('/') {
        return Err(anyhow!(
            "component name {component_name:?} must be a safe local Glyph identifier"
        ));
    }
    if !valid_source_ident(instance_name) || instance_name.contains('/') {
        return Err(anyhow!(
            "instance name {instance_name:?} must be a safe top-level Glyph identifier"
        ));
    }
    if direct_authored_component_span_records(source)?
        .iter()
        .any(|(name, _, _, _)| name == component_name)
    {
        return Err(anyhow!("component {component_name:?} already exists"));
    }
    if members.is_empty() {
        return Err(anyhow!(
            "component extraction requires at least one authored node"
        ));
    }

    let mut selected = Vec::new();
    let mut selected_ids = BTreeSet::new();
    for requested in members {
        let owner = resolve_layer_owner(source, requested)?;
        if owner.is_instance {
            return Err(anyhow!(
                "component extraction does not nest existing instances; {requested} is an instance owner"
            ));
        }
        if owner.kind == "group" {
            return Err(anyhow!(
                "component extraction v1 accepts directly-authored leaf nodes; extract group members explicitly"
            ));
        }
        if !selected_ids.insert(owner.id.clone()) {
            return Err(anyhow!("node {requested} was selected more than once"));
        }
        selected.push(owner);
    }
    selected.sort_by_key(|owner| owner.span_start);

    let stable_id = stable_id.unwrap_or(instance_name);
    if stable_id.is_empty() || stable_id.contains('/') {
        return Err(anyhow!(
            "instance id {stable_id:?} must be non-empty and must not contain '/'"
        ));
    }
    for owner in direct_layer_owners(source)? {
        if selected_ids.contains(&owner.id) {
            continue;
        }
        if owner.id == stable_id
            || owner.name == instance_name
            || owner.id == instance_name
            || owner.name == stable_id
        {
            return Err(anyhow!(
                "extracted instance {instance_name:?} / id {stable_id:?} collides with existing layer owner {}",
                owner.id
            ));
        }
    }

    let render_owners = direct_layer_owners(source)?
        .into_iter()
        .filter(|owner| owner.is_instance || owner.kind != "group")
        .collect::<Vec<_>>();
    let positions = selected
        .iter()
        .map(|selected_owner| {
            render_owners
                .iter()
                .position(|owner| owner.id == selected_owner.id && !owner.is_instance)
                .ok_or_else(|| {
                    anyhow!(
                        "selected node {} disappeared from layer order",
                        selected_owner.id
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;
    if positions
        .windows(2)
        .any(|pair| pair[1] != pair[0].saturating_add(1))
    {
        return Err(anyhow!(
            "component extraction requires contiguous rendered layers so paint order remains exact"
        ));
    }

    let selected_symbols = selected
        .iter()
        .flat_map(|owner| [owner.id.clone(), owner.name.clone()])
        .collect::<BTreeSet<_>>();
    for owner in &selected {
        let references = authored_node_reference_count(source, &owner.name)
            + if owner.id == owner.name {
                0
            } else {
                authored_node_reference_count(source, &owner.id)
            };
        if references > 0 {
            return Err(anyhow!(
                "node {} has {references} authored reference(s); component extraction refuses to change its source namespace implicitly",
                owner.id
            ));
        }
        let (start, end) = layer_owner_span(source, owner)?;
        let block = &source[start..end];
        if ["clip", "mask", "a11y_labelled_by"]
            .iter()
            .any(|property| authored_property_target(block, property).is_some())
        {
            return Err(anyhow!(
                "node {} has an authored cross-node property reference; component extraction requires explicit reference ownership",
                owner.id
            ));
        }
    }
    for owner in direct_layer_owners(source)? {
        if owner.is_instance || selected_ids.contains(&owner.id) {
            continue;
        }
        let (start, end) = layer_owner_span(source, &owner)?;
        let block = &source[start..end];
        for property in ["clip", "mask", "a11y_labelled_by"] {
            if let Some(target) = authored_property_target(block, property)
                && selected_symbols.contains(&target)
            {
                return Err(anyhow!(
                    "node {} references selected node {target} through {property}; extraction would change that namespace",
                    owner.id
                ));
            }
        }
    }

    let first_owner = selected
        .first()
        .ok_or_else(|| anyhow!("component extraction lost its selection"))?;
    let first_start = layer_owner_span(source, first_owner)?.0;
    let line_start = source[..first_owner.span_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let outer_indent = source[line_start..first_owner.span_start].to_string();
    let child_indent = format!("{outer_indent}  ");
    let mut component = format!("{outer_indent}component {component_name} {{\n");
    for owner in &selected {
        let (start, end) = layer_owner_span(source, owner)?;
        component.push_str(&reindent_block(&source[start..end], &child_indent));
        component.push('\n');
    }
    component.push_str(&format!("{outer_indent}}}\n"));
    let id_attr = if stable_id == instance_name {
        String::new()
    } else {
        format!(" @id(\"{}\")", stable_id.replace('"', "\\\""))
    };
    component.push_str(&format!(
        "{outer_indent}instance {component_name} {instance_name}{id_attr} {{ }}\n"
    ));

    let mut out = source.to_string();
    let mut spans = selected
        .iter()
        .map(|owner| layer_owner_span(source, owner))
        .collect::<Result<Vec<_>>>()?;
    spans.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    for (start, end) in spans {
        out.replace_range(start..end, "");
    }
    out.insert_str(first_start, &component);
    validate_structural_group_source(&out)?;
    Ok((out, stable_id.to_string()))
}

pub fn create_source_instance(
    source: &str,
    component: &str,
    name: &str,
    stable_id: Option<&str>,
    properties: &[(String, Value)],
    parameters: &[(String, Value)],
    slots: &[(String, Value)],
    variant: Option<&str>,
) -> Result<String> {
    if !valid_source_ident(component) {
        return Err(anyhow!(
            "component {component:?} is not a valid Glyph identifier"
        ));
    }
    if !valid_source_ident(name) || name.contains('/') {
        return Err(anyhow!(
            "instance name {name:?} must be a safe top-level Glyph identifier"
        ));
    }
    let stable_id = stable_id.unwrap_or(name);
    if stable_id.is_empty() || stable_id.contains('/') {
        return Err(anyhow!(
            "instance id {stable_id:?} must be non-empty and must not contain '/'"
        ));
    }
    if direct_layer_owners(source)?.iter().any(|owner| {
        owner.id == stable_id || owner.name == name || owner.id == name || owner.name == stable_id
    }) {
        return Err(anyhow!(
            "instance {name:?} / id {stable_id:?} collides with an existing layer owner"
        ));
    }
    for (binding_name, _) in parameters.iter().chain(slots.iter()) {
        if !valid_source_ident(binding_name) || binding_name.contains('/') {
            return Err(anyhow!(
                "binding name {binding_name:?} is not a safe identifier"
            ));
        }
    }
    if let Some(variant) = variant
        && (!valid_source_ident(variant) || variant.contains('/'))
    {
        return Err(anyhow!("variant {variant:?} is not a safe identifier"));
    }

    let id_attr = if stable_id == name {
        String::new()
    } else {
        format!(" @id(\"{}\")", stable_id.replace('"', "\\\""))
    };
    let mut declaration = format!("instance {component} {name}{id_attr} {{");
    for (property, value) in properties {
        if !valid_source_ident(property) || property.contains('/') {
            return Err(anyhow!(
                "instance property {property:?} is not a safe identifier"
            ));
        }
        declaration.push_str(&format!(" {property} {};", encode_value(value)));
    }
    for (parameter, value) in parameters {
        declaration.push_str(&format!(" param {parameter} {};", encode_value(value)));
    }
    for (slot, value) in slots {
        declaration.push_str(&format!(" slot {slot} {};", encode_value(value)));
    }
    if let Some(variant) = variant {
        declaration.push_str(&format!(" variant {variant};"));
    }
    declaration.push_str(" }");

    let (_, scene_end) = direct_authored_scene_source_span(source)?;
    let close = scene_end
        .checked_sub(1)
        .ok_or_else(|| anyhow!("invalid scene source span"))?;
    let line_start = source[..close]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let closing_indent = if source[line_start..close].trim().is_empty() {
        &source[line_start..close]
    } else {
        ""
    };
    let item_indent = format!("{closing_indent}  ");
    let insertion = format!("{item_indent}{declaration}\n");
    let mut out = source.to_string();
    if source[line_start..close].trim().is_empty() {
        out.insert_str(line_start, &insertion);
    } else {
        out.insert_str(close, &format!("\n{insertion}"));
    }
    validate_source_syntax(&out)?;
    Ok(out)
}

pub fn remove_source_instance(source: &str, instance_id: &str) -> Result<String> {
    let (span_start, span_end) = direct_authored_instance_source_span(source, instance_id)?;
    let (start, _, close) = block_from_source_span(source, span_start, span_end, instance_id)?;
    let out = remove_statement_or_line(source, start, close + 1);
    validate_source_syntax(&out)?;
    Ok(out)
}

fn direct_node_reference(source: &str, requested: &str) -> Result<(String, String, String)> {
    let records = direct_authored_node_records(source)?;
    let matches = records
        .iter()
        .filter(|(id, name, _, _)| id == requested || name == requested)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(anyhow!(
            "node {requested} is not one unambiguous directly-authored node"
        ));
    }
    let (id, name, kind, _) = matches[0];
    for candidate in [id.as_str(), name.as_str()] {
        if !valid_source_ident(candidate) {
            continue;
        }
        let owners = records
            .iter()
            .filter(|(other_id, other_name, _, _)| other_id == candidate || other_name == candidate)
            .count();
        if owners == 1 {
            return Ok((id.clone(), candidate.to_string(), kind.clone()));
        }
    }
    Err(anyhow!(
        "node {requested} has no unambiguous identifier usable by structural group source"
    ))
}

pub fn move_source_node(
    source: &str,
    node_id: &str,
    anchor_id: &str,
    placement: NodePlacement,
) -> Result<String> {
    let owner = resolve_layer_owner(source, node_id)?;
    let anchor = resolve_layer_owner(source, anchor_id)?;
    if owner.id == anchor.id && owner.is_instance == anchor.is_instance {
        return Err(anyhow!("cannot move node {node_id} relative to itself"));
    }
    if !owner.is_instance && owner.kind == "group" {
        return Err(anyhow!(
            "structural group {} has no independent paint order; reorder its rendered members instead",
            owner.id
        ));
    }
    if !anchor.is_instance && anchor.kind == "group" {
        return Err(anyhow!(
            "structural group {} cannot be a paint-order anchor; choose one of its rendered members",
            anchor.id
        ));
    }

    let before_owners = direct_layer_owners(source)?
        .into_iter()
        .map(|candidate| (candidate.id, candidate.is_instance))
        .collect::<Vec<_>>();
    let (start, end) = layer_owner_span(source, &owner)?;
    let block = source[start..end].to_string();
    let mut without = source.to_string();
    without.replace_range(start..end, "");

    let anchor_after_removal = resolve_layer_owner(&without, &anchor.id)?;
    let (anchor_start, anchor_end) = layer_owner_span(&without, &anchor_after_removal)?;
    let insertion = match placement {
        NodePlacement::Before => anchor_start,
        NodePlacement::After => anchor_end,
    };
    without.insert_str(insertion, &block);
    validate_structural_group_source(&without)?;

    let after_owners = direct_layer_owners(&without)?
        .into_iter()
        .map(|candidate| (candidate.id, candidate.is_instance))
        .collect::<Vec<_>>();
    if before_owners.len() != after_owners.len()
        || before_owners
            .iter()
            .any(|owner| !after_owners.contains(owner))
        || after_owners
            .iter()
            .any(|owner| !before_owners.contains(owner))
    {
        return Err(anyhow!(
            "node reorder changed the directly-authored layer-owner set; refusing source edit"
        ));
    }
    Ok(without)
}

fn group_member_spans(source: &str, group_id: &str) -> Result<Vec<(String, usize, usize)>> {
    let (_, _, kind) = direct_node_reference(source, group_id)?;
    if kind != "group" {
        return Err(anyhow!("node {group_id} is not a structural group"));
    }
    let (_, open, close) = node_block(source, group_id)?;
    let body = &source[open + 1..close];
    let bytes = body.as_bytes();
    let mut members = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i = skip_line_comment(bytes, i + 2);
            continue;
        }
        if !token_at(body, i, "member") {
            i += 1;
            continue;
        }
        let statement_start = i;
        let mut cursor = i + "member".len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let reference_start = cursor;
        while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b';'
        {
            cursor += 1;
        }
        if reference_start == cursor {
            return Err(anyhow!("group {group_id} contains malformed member source"));
        }
        let reference = body[reference_start..cursor].to_string();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b';') {
            return Err(anyhow!(
                "group {group_id} member {reference} is missing ';'"
            ));
        }
        cursor += 1;
        members.push((reference, open + 1 + statement_start, open + 1 + cursor));
        i = cursor;
    }
    Ok(members)
}

fn validate_structural_group_source(source: &str) -> Result<()> {
    validate_source_syntax(source)?;
    if parse_import_specs(source)?.is_empty() {
        parse_scene(source, "<structural-group-edit>")?;
    }
    Ok(())
}

fn group_owners_for_direct_node(source: &str, node_id: &str) -> Result<Vec<String>> {
    let (stable_id, reference, _) = direct_node_reference(source, node_id)?;
    let (_, name, _) = direct_authored_node_identity(source, node_id)?;
    let mut owners = Vec::new();
    for (group_id, _, kind, _) in direct_authored_node_records(source)? {
        if kind != "group" || group_id == stable_id {
            continue;
        }
        if group_member_spans(source, &group_id)?
            .iter()
            .any(|(member, _, _)| member == &stable_id || member == &name || member == &reference)
        {
            owners.push(group_id);
        }
    }
    Ok(owners)
}

fn next_group_name(source: &str) -> Result<String> {
    let records = direct_authored_node_records(source)?;
    for ordinal in 1usize.. {
        let candidate = if ordinal == 1 {
            "group".to_string()
        } else {
            format!("group_{ordinal}")
        };
        if !records
            .iter()
            .any(|(id, name, _, _)| id == &candidate || name == &candidate)
        {
            return Ok(candidate);
        }
    }
    unreachable!()
}

pub fn create_source_group(
    source: &str,
    requested_name: Option<&str>,
    members: &[String],
) -> Result<(String, String)> {
    if members.len() < 2 {
        return Err(anyhow!(
            "creating a structural group requires at least two members"
        ));
    }
    let group_name = match requested_name {
        Some(name) => {
            if !valid_source_ident(name) || name.contains('/') {
                return Err(anyhow!(
                    "group name {name:?} is not a safe top-level Glyph identifier"
                ));
            }
            let records = direct_authored_node_records(source)?;
            if records
                .iter()
                .any(|(id, node_name, _, _)| id == name || node_name == name)
            {
                return Err(anyhow!("top-level node {name:?} already exists"));
            }
            name.to_string()
        }
        None => next_group_name(source)?,
    };

    let mut references = Vec::new();
    let mut stable_ids = Vec::new();
    for member in members {
        let (stable_id, reference, kind) = direct_node_reference(source, member)?;
        if stable_id == group_name {
            return Err(anyhow!("group {group_name} cannot contain itself"));
        }
        if stable_ids.iter().any(|id| id == &stable_id) {
            return Err(anyhow!("group member {member} was selected more than once"));
        }
        let owners = group_owners_for_direct_node(source, &stable_id)?;
        if !owners.is_empty() {
            return Err(anyhow!(
                "node {member} already belongs to structural group(s): {}",
                owners.join(", ")
            ));
        }
        if kind == "group" {
            // Nested groups are valid, but ownership still remains singular.
        }
        stable_ids.push(stable_id);
        references.push(reference);
    }

    let (_, scene_end) = direct_authored_scene_source_span(source)?;
    let close = scene_end
        .checked_sub(1)
        .ok_or_else(|| anyhow!("invalid scene source span"))?;
    if source.as_bytes().get(close) != Some(&b'}') {
        return Err(anyhow!("scene source span does not end at closing brace"));
    }
    let line_start = source[..close]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let closing_indent = if source[line_start..close].trim().is_empty() {
        &source[line_start..close]
    } else {
        ""
    };
    let item_indent = format!("{closing_indent}  ");
    let member_indent = format!("{item_indent}  ");
    let mut declaration = format!("{item_indent}group {group_name} {{\n");
    for reference in &references {
        declaration.push_str(&format!("{member_indent}member {reference};\n"));
    }
    declaration.push_str(&format!("{item_indent}}}\n"));

    let mut out = source.to_string();
    if source[line_start..close].trim().is_empty() {
        out.insert_str(line_start, &declaration);
    } else {
        out.insert_str(close, &format!("\n{declaration}"));
    }
    validate_structural_group_source(&out)?;
    Ok((out, group_name))
}

fn remove_statement_or_line(source: &str, start: usize, end: usize) -> String {
    let line_start = source[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = source[end..]
        .find('\n')
        .map(|index| end + index + 1)
        .unwrap_or(end);
    let isolated =
        source[line_start..start].trim().is_empty() && source[end..line_end].trim().is_empty();
    let mut out = source.to_string();
    if isolated {
        out.replace_range(line_start..line_end, "");
    } else {
        out.replace_range(start..end, "");
    }
    out
}

pub fn add_source_group_member(source: &str, group_id: &str, member_id: &str) -> Result<String> {
    let (_, _, group_kind) = direct_node_reference(source, group_id)?;
    if group_kind != "group" {
        return Err(anyhow!("node {group_id} is not a structural group"));
    }
    let (stable_id, reference, _) = direct_node_reference(source, member_id)?;
    let (group_stable_id, _, _) = direct_node_reference(source, group_id)?;
    if stable_id == group_stable_id {
        return Err(anyhow!("group {group_id} cannot contain itself"));
    }
    let owners = group_owners_for_direct_node(source, &stable_id)?;
    if !owners.is_empty() {
        return Err(anyhow!(
            "node {member_id} already belongs to structural group(s): {}",
            owners.join(", ")
        ));
    }
    let (_, open, close) = node_block(source, group_id)?;
    let header_line_start = source[..open]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let header_indent = source[header_line_start..open]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let insertion = format!("\n{header_indent}  member {reference};\n{header_indent}");
    let mut out = source.to_string();
    out.insert_str(close, &insertion);
    validate_structural_group_source(&out)?;
    Ok(out)
}

pub fn remove_source_group_member(source: &str, group_id: &str, member_id: &str) -> Result<String> {
    let (stable_id, reference, _) = direct_node_reference(source, member_id)?;
    let (_, name, _) = direct_authored_node_identity(source, member_id)?;
    let matches = group_member_spans(source, group_id)?
        .into_iter()
        .filter(|(member, _, _)| member == &stable_id || member == &name || member == &reference)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(anyhow!(
            "node {member_id} is not exactly one member of structural group {group_id}"
        ));
    }
    let (_, start, end) = matches[0].clone();
    let out = remove_statement_or_line(source, start, end);
    validate_structural_group_source(&out)?;
    Ok(out)
}

pub fn reparent_source_node(
    source: &str,
    node_id: &str,
    new_parent: Option<&str>,
) -> Result<String> {
    let (stable_id, _, _) = direct_node_reference(source, node_id).map_err(|_| {
        anyhow!(
            "node {node_id} is not a directly-authored source node that can participate in structural hierarchy"
        )
    })?;
    let owners = group_owners_for_direct_node(source, &stable_id)?;
    if owners.len() > 1 {
        return Err(anyhow!(
            "node {node_id} belongs to multiple structural groups: {}",
            owners.join(", ")
        ));
    }
    let current_parent = owners.into_iter().next();

    let target_parent = match new_parent {
        Some(parent) => {
            let (parent_id, _, kind) = direct_node_reference(source, parent)?;
            if kind != "group" {
                return Err(anyhow!("node {parent} is not a structural group"));
            }
            if parent_id == stable_id {
                return Err(anyhow!("group {node_id} cannot contain itself"));
            }
            Some(parent_id)
        }
        None => None,
    };

    if current_parent == target_parent {
        return Ok(source.to_string());
    }

    let mut edited = source.to_string();
    if let Some(parent) = current_parent {
        edited = remove_source_group_member(&edited, &parent, &stable_id)?;
    }
    if let Some(parent) = target_parent {
        edited = add_source_group_member(&edited, &parent, &stable_id)?;
    }
    validate_structural_group_source(&edited)?;
    Ok(edited)
}

pub fn remove_source_group(source: &str, group_id: &str) -> Result<String> {
    let (stable_id, name, kind) = direct_node_reference(source, group_id)?;
    if kind != "group" {
        return Err(anyhow!("node {group_id} is not a structural group"));
    }
    for (other_id, _, other_kind, _) in direct_authored_node_records(source)? {
        if other_kind != "group" || other_id == stable_id {
            continue;
        }
        if group_member_spans(source, &other_id)?
            .iter()
            .any(|(member, _, _)| member == &stable_id || member == &name)
        {
            return Err(anyhow!(
                "group {group_id} is owned by parent group {other_id}; remove that membership first"
            ));
        }
    }
    let (start, _, close) = node_block(source, group_id)?;
    let out = remove_statement_or_line(source, start, close + 1);
    validate_structural_group_source(&out)?;
    Ok(out)
}
