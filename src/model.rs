use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub schema_version: u32,
    pub revision: u64,
    pub id: String,
    pub name: String,
    pub canvas: Canvas,
    pub nodes: Vec<Node>,
    pub constraints: Vec<Constraint>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Rect,
    Ellipse,
    Path,
    Text,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub roles: Vec<String>,
    pub authored: AuthoredNode,
    pub derived: DerivedNode,
    pub provenance: Provenance,
    pub confidence: Option<Confidence>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthoredNode {
    pub props: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DerivedNode {
    pub props: BTreeMap<String, f64>,
    pub relaxed_constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub source: String,
    pub line: usize,
    pub authored: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confidence {
    pub semantics: Option<f64>,
    pub geometry: Option<f64>,
    pub depth: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    Text(String),
}

impl Value {
    pub fn number(&self) -> Option<f64> {
        if let Self::Number(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub lhs: PropertyRef,
    pub relation: Relation,
    pub rhs: ConstraintRhs,
    pub offset: f64,
    pub strength: Strength,
    pub source_order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PropertyRef {
    pub node: String,
    pub property: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum ConstraintRhs {
    Property(PropertyRef),
    Number(f64),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Relation {
    Eq,
    Ge,
    Le,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    Weak,
    Medium,
    Strong,
    Required,
}

impl Strength {
    pub fn rank(self) -> u8 {
        match self {
            Self::Weak => 1,
            Self::Medium => 2,
            Self::Strong => 3,
            Self::Required => 4,
        }
    }
}
