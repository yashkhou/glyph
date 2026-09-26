use crate::model::*;
use anyhow::{Context, Result, anyhow};
use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser;
use std::collections::BTreeMap;

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

fn parse_value(pair: Pair<Rule>) -> Result<Value> {
    match pair.as_rule() {
        Rule::number => Ok(Value::Number(pair.as_str().parse()?)),
        Rule::string => Ok(Value::Text(unquote(pair.as_str()))),
        Rule::ident => Ok(Value::Text(pair.as_str().to_string())),
        _ => Err(anyhow!("unsupported value {:?}", pair.as_rule())),
    }
}

fn parse_node(pair: Pair<Rule>, source: &str) -> Result<Node> {
    let line = line_of(&pair);
    let mut inner = pair.into_inner();
    let kind = match inner.next().context("missing node kind")?.as_str() {
        "rect" => NodeKind::Rect,
        "ellipse" => NodeKind::Ellipse,
        "path" => NodeKind::Path,
        "text" => NodeKind::Text,
        "group" => NodeKind::Group,
        other => return Err(anyhow!("unsupported node kind {other}")),
    };
    let name = inner
        .next()
        .context("missing node name")?
        .as_str()
        .to_string();
    let mut id = name.clone();
    let mut roles = Vec::new();
    let mut props = BTreeMap::new();

    for item in inner {
        match item.as_rule() {
            Rule::id_attr => {
                let raw = item.into_inner().next().context("missing id string")?;
                id = unquote(raw.as_str());
            }
            Rule::role => {
                roles.extend(
                    item.into_inner()
                        .filter(|p| p.as_rule() == Rule::string)
                        .map(|p| unquote(p.as_str())),
                );
            }
            Rule::prop => {
                let mut bits = item.into_inner();
                let key = bits
                    .next()
                    .context("missing property name")?
                    .as_str()
                    .to_string();
                let values: Vec<Value> = bits.map(parse_value).collect::<Result<_>>()?;
                if values.len() == 1 {
                    props.insert(key, values.into_iter().next().unwrap());
                } else {
                    let joined = values
                        .into_iter()
                        .map(|v| match v {
                            Value::Number(n) => n.to_string(),
                            Value::Text(s) => s,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    props.insert(key, Value::Text(joined));
                }
            }
            _ => {}
        }
    }

    Ok(Node {
        id,
        name,
        kind,
        roles,
        authored: AuthoredNode { props },
        derived: DerivedNode::default(),
        provenance: Provenance {
            source: source.to_string(),
            line,
            authored: true,
        },
        confidence: None,
    })
}

fn parse_constraint(pair: Pair<Rule>, index: usize) -> Result<Constraint> {
    let mut inner = pair.into_inner();
    let lhs_pair = inner.next().context("missing constraint lhs")?;
    let lhs = property_ref(lhs_pair.as_str())?;
    let relation = match inner.next().context("missing relation")?.as_str() {
        "==" => Relation::Eq,
        ">=" => Relation::Ge,
        "<=" => Relation::Le,
        other => return Err(anyhow!("unsupported relation {other}")),
    };
    let rhs_pair = inner.next().context("missing rhs")?;
    let rhs_inner = rhs_pair.into_inner().next().context("empty rhs")?;
    let rhs = match rhs_inner.as_rule() {
        Rule::prop_ref => ConstraintRhs::Property(property_ref(rhs_inner.as_str())?),
        Rule::number => ConstraintRhs::Number(rhs_inner.as_str().parse()?),
        _ => return Err(anyhow!("bad rhs")),
    };
    let mut offset = 0.0;
    let mut strength = Strength::Required;
    for item in inner {
        match item.as_rule() {
            Rule::offset => {
                let mut parts = item.into_inner();
                let sign = parts.next().context("missing offset sign")?.as_str();
                let value: f64 = parts
                    .next()
                    .context("missing offset value")?
                    .as_str()
                    .parse()?;
                offset = if sign == "-" { -value } else { value };
            }
            Rule::strength => {
                strength = match item.as_str() {
                    "@required" => Strength::Required,
                    "@strong" => Strength::Strong,
                    "@medium" => Strength::Medium,
                    "@weak" => Strength::Weak,
                    _ => Strength::Required,
                };
            }
            _ => {}
        }
    }
    Ok(Constraint {
        id: format!("constraint.{index}"),
        lhs,
        relation,
        rhs,
        offset,
        strength,
        source_order: index,
    })
}

pub fn parse_scene(input: &str, source: &str) -> Result<Scene> {
    let mut parsed = GlyphParser::parse(Rule::file, input).context("Glyph parse failed")?;
    let file = parsed.next().context("missing file")?;
    let scene_pair = file.into_inner().next().context("missing scene")?;
    let mut inner = scene_pair.into_inner();
    let name = unquote(inner.next().context("missing scene name")?.as_str());
    let mut id = name.to_lowercase().replace(' ', "-");
    let mut canvas = Canvas {
        width: 1024.0,
        height: 1024.0,
        background: None,
    };
    let mut nodes = Vec::new();
    let mut constraints = Vec::new();

    for item in inner {
        match item.as_rule() {
            Rule::id_attr => {
                id = unquote(
                    item.into_inner()
                        .next()
                        .context("missing scene id")?
                        .as_str(),
                );
            }
            Rule::canvas => {
                let mut bits = item.into_inner();
                canvas.width = bits
                    .next()
                    .context("missing canvas width")?
                    .as_str()
                    .parse()?;
                canvas.height = bits
                    .next()
                    .context("missing canvas height")?
                    .as_str()
                    .parse()?;
                if let Some(bg) = bits.next() {
                    canvas.background = Some(unquote(bg.as_str()));
                }
            }
            Rule::node => nodes.push(parse_node(item, source)?),
            Rule::constraint => constraints.push(parse_constraint(item, constraints.len() + 1)?),
            _ => {}
        }
    }

    let mut metadata = BTreeMap::new();
    metadata.insert("engine".into(), "Glyph Core".into());
    metadata.insert("source".into(), source.into());
    Ok(Scene {
        schema_version: 1,
        revision: 1,
        id,
        name,
        canvas,
        nodes,
        constraints,
        metadata,
    })
}
