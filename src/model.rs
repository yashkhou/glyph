use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CURRENT_SOURCE_VERSION: u32 = 1;
pub const CURRENT_GSIR_SCHEMA_VERSION: u32 = 2;

fn default_source_version() -> u32 {
    CURRENT_SOURCE_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportSpec {
    pub path: String,
    pub alias: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub schema_version: u32,
    #[serde(default = "default_source_version")]
    pub source_version: u32,
    pub revision: u64,
    #[serde(default)]
    pub imports: Vec<ImportSpec>,
    pub id: String,
    pub name: String,
    pub canvas: Canvas,
    pub nodes: Vec<Node>,
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    pub assertions: Vec<Assertion>,
    #[serde(default)]
    pub styles: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub paints: BTreeMap<String, Paint>,
    #[serde(default)]
    pub components: Vec<ComponentDefinition>,
    #[serde(default)]
    pub animations: Vec<AnimationSpec>,
    #[serde(default)]
    pub states: Vec<SceneState>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_unit: Option<Unit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_unit: Option<Unit>,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Rect,
    Ellipse,
    Path,
    Text,
    Image,
    Group,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<String>,
    pub authored: AuthoredNode,
    pub derived: DerivedNode,
    pub provenance: Provenance,
    pub confidence: Option<Confidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_origin: Option<ComponentInstanceOrigin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInstanceOrigin {
    pub instance_id: String,
    pub instance_name: String,
    pub component: String,
    pub local_id: String,
    pub local_name: String,
    pub offset_x: f64,
    pub offset_y: f64,
    pub scale: f64,
    #[serde(default)]
    pub parameter_bindings: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub overridden_properties: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthoredNode {
    pub props: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DerivedNode {
    pub props: BTreeMap<String, f64>,
    pub relaxed_constraints: Vec<String>,
    #[serde(default)]
    pub diagnostics: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GradientStop {
    pub offset: f64,
    pub color: String,
    pub opacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Paint {
    Linear {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        stops: Vec<GradientStop>,
    },
    Radial {
        cx: f64,
        cy: f64,
        r: f64,
        stops: Vec<GradientStop>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Number,
    Length,
    Angle,
    Ratio,
    Depth,
    Time,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Px,
    Deg,
    Rad,
    Percent,
    S,
    Ms,
}

impl Unit {
    pub fn dimension(self) -> Dimension {
        match self {
            Self::Px => Dimension::Length,
            Self::Deg | Self::Rad => Dimension::Angle,
            Self::Percent => Dimension::Ratio,
            Self::S | Self::Ms => Dimension::Time,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::Px => "px",
            Self::Deg => "deg",
            Self::Rad => "rad",
            Self::Percent => "%",
            Self::S => "s",
            Self::Ms => "ms",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
}

impl Quantity {
    pub fn dimension(self) -> Dimension {
        self.unit.dimension()
    }

    pub fn canonical_value(self) -> f64 {
        match self.unit {
            Unit::Px | Unit::Deg | Unit::S => self.value,
            Unit::Rad => self.value.to_degrees(),
            Unit::Percent => self.value / 100.0,
            Unit::Ms => self.value / 1000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    Number,
    Length,
    Angle,
    Color,
    Opacity,
    Percentage,
    Depth,
    Time,
    Point,
    Vector,
    Rect,
    Path,
    Shape,
    Image,
    Texture,
    Font,
    Object,
    Region,
    Constraint,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Number(f64),
    Quantity(Quantity),
    Text(String),
}

impl Value {
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            Self::Quantity(v) => Some(v.canonical_value()),
            Self::Text(_) => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn explicit_dimension(&self) -> Option<Dimension> {
        match self {
            Self::Quantity(v) => Some(v.dimension()),
            Self::Number(_) | Self::Text(_) => None,
        }
    }
}

impl ValueType {
    pub fn dimension(self) -> Option<Dimension> {
        match self {
            Self::Number => Some(Dimension::Number),
            Self::Length => Some(Dimension::Length),
            Self::Angle => Some(Dimension::Angle),
            Self::Opacity | Self::Percentage => Some(Dimension::Ratio),
            Self::Depth => Some(Dimension::Depth),
            Self::Time => Some(Dimension::Time),
            _ => None,
        }
    }

    pub fn from_name(raw: &str) -> Option<Self> {
        Some(match raw {
            "Number" => Self::Number,
            "Length" => Self::Length,
            "Angle" => Self::Angle,
            "Color" => Self::Color,
            "Opacity" => Self::Opacity,
            "Percentage" => Self::Percentage,
            "Depth" => Self::Depth,
            "Time" => Self::Time,
            "Point" => Self::Point,
            "Vector" => Self::Vector,
            "Rect" => Self::Rect,
            "Path" => Self::Path,
            "Shape" => Self::Shape,
            "Image" => Self::Image,
            "Texture" => Self::Texture,
            "Font" => Self::Font,
            "Object" => Self::Object,
            "Region" => Self::Region,
            "Constraint" => Self::Constraint,
            "Text" => Self::Text,
            _ => return None,
        })
    }
}

pub fn property_value_type(property: &str) -> Option<ValueType> {
    if property.starts_with("font_axis_") {
        return Some(ValueType::Number);
    }
    Some(match property {
        "x" | "y" | "cx" | "cy" | "width" | "height" | "rx" | "ry" | "left" | "right" | "top"
        | "bottom" | "center_x" | "center_y" | "size" | "max_width" | "line_height"
        | "stroke_width" | "dx" | "dy" | "jitter_x" | "jitter_y" | "ascent" | "translate_x"
        | "translate_y" | "origin_x" | "origin_y" | "descent" | "leading" | "baseline"
        | "letter_spacing" => ValueType::Length,
        "rotation" | "rotate" | "angle" => ValueType::Angle,
        "opacity" => ValueType::Opacity,
        "depth" | "z" => ValueType::Depth,
        "scale" | "scale_x" | "scale_y" | "scale_step" | "line_count" | "glyph_count"
        | "reading_order" => ValueType::Number,
        "fill" | "stroke" | "background" | "color" => ValueType::Color,
        "d" | "path" => ValueType::Path,
        "src" => ValueType::Image,
        "fit" | "clip" | "mask" | "fill_paint" | "stroke_paint" | "weight" | "font_style"
        | "ligatures" | "a11y_label" | "a11y_role" | "a11y_hint" | "a11y_labelled_by"
        | "a11y_hidden" | "a11y_live" | "interactive" | "action" => ValueType::Text,
        "font" => ValueType::Font,
        "content" => ValueType::Text,
        _ => return None,
    })
}

pub fn property_dimension(property: &str) -> Option<Dimension> {
    property_value_type(property).and_then(ValueType::dimension)
}

pub fn validate_value_for_property(property: &str, value: &Value) -> Result<(), String> {
    if let Some(tag) = property.strip_prefix("font_axis_") {
        if tag.len() != 4 || !tag.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!(
                "font variation axis tag {tag:?} must be exactly four ASCII alphanumeric characters"
            ));
        }
    }
    if property == "ligatures" {
        let Some(mode) = value.text() else {
            return Err("property ligatures expects text".into());
        };
        if !matches!(mode, "none" | "standard" | "all") {
            return Err(format!(
                "property ligatures expects none, standard, or all; got {mode:?}"
            ));
        }
    }
    if matches!(property, "a11y_hidden" | "interactive") {
        let Some(flag) = value.text() else {
            return Err(format!("property {property} expects true or false"));
        };
        if !matches!(flag, "true" | "false") {
            return Err(format!(
                "property {property} expects true or false; got {flag:?}"
            ));
        }
    }
    if property == "a11y_live" {
        let Some(mode) = value.text() else {
            return Err("property a11y_live expects off, polite, or assertive".into());
        };
        if !matches!(mode, "off" | "polite" | "assertive") {
            return Err(format!(
                "property a11y_live expects off, polite, or assertive; got {mode:?}"
            ));
        }
    }
    if property == "reading_order" {
        let Some(order) = value.number() else {
            return Err("property reading_order expects a non-negative integer".into());
        };
        if !order.is_finite() || order < 0.0 || order.fract().abs() > f64::EPSILON {
            return Err(format!(
                "property reading_order expects a non-negative integer; got {order}"
            ));
        }
    }
    let Some(expected) = property_value_type(property) else {
        if let Some(actual) = value.explicit_dimension() {
            return Err(format!(
                "property {property} has no declared dimensional type; cannot apply explicit {actual:?} value"
            ));
        }
        return Ok(());
    };
    validate_value_type(expected, value).map_err(|message| format!("property {property} {message}"))
}

pub fn validate_value_type(expected: ValueType, value: &Value) -> Result<(), String> {
    match expected {
        ValueType::Length
        | ValueType::Angle
        | ValueType::Opacity
        | ValueType::Percentage
        | ValueType::Time => {
            let expected_dimension = expected.dimension().expect("scalar type has dimension");
            match value {
                Value::Text(_) => Err(format!("expects {expected:?}, got text")),
                Value::Quantity(q) if q.dimension() != expected_dimension => Err(format!(
                    "expects {expected_dimension:?}, got {:?} ({})",
                    q.dimension(),
                    q.unit.suffix()
                )),
                Value::Number(_) | Value::Quantity(_) => Ok(()),
            }
        }
        ValueType::Number | ValueType::Depth => match value {
            Value::Number(_) => Ok(()),
            Value::Quantity(q) => Err(format!(
                "expects {expected:?}, got explicit {:?} unit {}",
                q.dimension(),
                q.unit.suffix()
            )),
            Value::Text(_) => Err(format!("expects {expected:?}, got text")),
        },
        ValueType::Color
        | ValueType::Path
        | ValueType::Image
        | ValueType::Texture
        | ValueType::Font
        | ValueType::Text => match value {
            Value::Text(_) => Ok(()),
            Value::Number(_) | Value::Quantity(_) => {
                Err(format!("expects {expected:?}, got numeric value"))
            }
        },
        ValueType::Point
        | ValueType::Vector
        | ValueType::Rect
        | ValueType::Shape
        | ValueType::Object
        | ValueType::Region
        | ValueType::Constraint => Err(format!(
            "expects {expected:?}, which has no scalar literal representation yet"
        )),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBinding {
    pub name: String,
    pub value_type: ValueType,
    pub target: PropertyRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentVariant {
    pub name: String,
    pub overrides: Vec<StateOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<ComponentBinding>,
    #[serde(default)]
    pub slots: Vec<ComponentBinding>,
    #[serde(default)]
    pub variants: Vec<ComponentVariant>,
    pub nodes: Vec<Node>,
}

fn default_animation_repeat() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationSpec {
    pub target: PropertyRef,
    pub from: f64,
    pub to: f64,
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_unit: Option<Unit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_unit: Option<Unit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_unit: Option<Unit>,
    #[serde(default)]
    pub delay: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_unit: Option<Unit>,
    #[serde(default = "default_animation_repeat")]
    pub repeat: u32,
    #[serde(default)]
    pub alternate: bool,
    pub easing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateOverride {
    pub target: PropertyRef,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneState {
    pub name: String,
    pub overrides: Vec<StateOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub id: String,
    pub lhs: PropertyRef,
    pub relation: Relation,
    pub rhs: ConstraintRhs,
    pub offset: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_unit: Option<Unit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub lhs: PropertyRef,
    pub relation: Relation,
    pub rhs: ConstraintRhs,
    pub offset: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_unit: Option<Unit>,
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
    Quantity(Quantity),
}

impl ConstraintRhs {
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            Self::Quantity(v) => Some(v.canonical_value()),
            Self::Property(_) => None,
        }
    }

    pub fn explicit_dimension(&self) -> Option<Dimension> {
        match self {
            Self::Quantity(v) => Some(v.dimension()),
            Self::Number(_) | Self::Property(_) => None,
        }
    }
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
