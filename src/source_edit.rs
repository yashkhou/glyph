use crate::{
    model::{
        ConstraintRhs, PropertyRef, Relation, Strength, Unit, Value, validate_value_for_property,
    },
    parser::{
        direct_authored_constraint_source_span, direct_authored_instance_identity,
        direct_authored_node_identity, direct_authored_scene_source_span, parse_scene,
        validate_source_syntax,
    },
};
use anyhow::{Result, anyhow};

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
fn node_block(source: &str, node_id: &str) -> Result<(usize, usize, usize)> {
    let (node_id, node_name, node_line) = direct_authored_node_identity(source, node_id)?;
    let lines: Vec<usize> = {
        let mut v = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                v.push(i + 1)
            }
        }
        v
    };
    let approx = *lines.get(node_line.saturating_sub(1)).unwrap_or(&0);
    let start = source[..=approx.min(source.len().saturating_sub(1))]
        .rfind(|c: char| c == '\n')
        .map(|x| x + 1)
        .unwrap_or(0);
    let mut i = start;
    let bytes = source.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i = skip_line_comment(bytes, i + 2);
            continue;
        }
        if bytes[i] == b'{' {
            let close = find_matching_brace(source, i)?;
            let header = &source[start..i];
            if header.contains(&format!("@id(\"{}\")", node_id))
                || (node_id == node_name && header.split_whitespace().any(|x| x == node_name))
            {
                return Ok((start, i, close));
            }
            i = close + 1;
            continue;
        }
        i += 1;
    }
    Err(anyhow!("source block for {node_id} not found"))
}

fn instance_block(source: &str, instance_id: &str) -> Result<(usize, usize, usize)> {
    let (instance_id, instance_name, instance_line) =
        direct_authored_instance_identity(source, instance_id)?;
    let lines: Vec<usize> = {
        let mut v = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                v.push(i + 1)
            }
        }
        v
    };
    let approx = *lines.get(instance_line.saturating_sub(1)).unwrap_or(&0);
    let start = source[..=approx.min(source.len().saturating_sub(1))]
        .rfind(|c: char| c == '\n')
        .map(|x| x + 1)
        .unwrap_or(0);
    let mut i = start;
    let bytes = source.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i = skip_string(bytes, i);
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            i = skip_line_comment(bytes, i + 2);
            continue;
        }
        if bytes[i] == b'{' {
            let close = find_matching_brace(source, i)?;
            let header = &source[start..i];
            if header.contains(&format!("@id(\"{}\")", instance_id))
                || (instance_id == instance_name
                    && header.split_whitespace().any(|x| x == instance_name))
            {
                return Ok((start, i, close));
            }
            i = close + 1;
            continue;
        }
        i += 1;
    }
    Err(anyhow!("source instance block for {instance_id} not found"))
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

pub fn set_source_instance_parameter(
    source: &str,
    instance_id: &str,
    parameter: &str,
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
        if token_at(body, i, "param") {
            let mut j = i + "param".len();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if token_at(body, j, parameter) {
                j += parameter.len();
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
    let insertion = format!(
        "\n{}param {} {};\n{}",
        item_indent, parameter, replacement, header_indent
    );
    let mut out = String::with_capacity(source.len() + insertion.len());
    out.push_str(&source[..close]);
    out.push_str(&insertion);
    out.push_str(&source[close..]);
    validate_source_syntax(&out)?;
    Ok(out)
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
