use crate::{model::*, query, text_layout::layout_text_node};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayList {
    pub schema_version: u32,
    pub scene_id: String,
    pub revision: u64,
    pub canvas: DisplayCanvas,
    pub items: Vec<DisplayItem>,
    pub object_map: BTreeMap<String, ObjectRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayCanvas {
    pub width: f64,
    pub height: f64,
    pub background: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayItem {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
    pub bounds: Option<[f64; 4]>,
    #[serde(default)]
    pub accessibility: DisplayAccessibility,
    pub primitive: DisplayPrimitive,
    pub style: DisplayStyle,
    pub transform: DisplayTransform,
    pub clip: Option<String>,
    pub mask: Option<String>,
    pub definition_only: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DisplayAccessibility {
    pub label: Option<String>,
    pub role: Option<String>,
    pub hint: Option<String>,
    pub labelled_by: Option<String>,
    pub live: Option<String>,
    pub hidden: bool,
    pub interactive: bool,
    pub reading_order: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayTransform {
    pub translate_x: f64,
    pub translate_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotate_deg: f64,
    pub origin_x: f64,
    pub origin_y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectRecord {
    pub index: usize,
    pub name: String,
    pub kind: String,
    pub roles: Vec<String>,
    pub bounds: Option<[f64; 4]>,
    #[serde(default)]
    pub accessibility: DisplayAccessibility,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplayStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub fill_paint: Option<Paint>,
    pub stroke_paint: Option<Paint>,
    pub stroke_width: f64,
    pub opacity: f64,
    pub fill_opacity: f64,
    pub stroke_opacity: f64,
    pub stroke_linecap: Option<String>,
    pub stroke_linejoin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DisplayPrimitive {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        rx: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
    },
    Path {
        d: String,
    },
    Text {
        x: f64,
        y: f64,
        lines: Vec<String>,
        line_widths: Vec<f64>,
        line_height: f64,
        ascent: f64,
        descent: f64,
        leading: f64,
        glyph_count: usize,
        font: String,
        requested_font: String,
        resolved_font: String,
        postscript_font: String,
        fallback_fonts: Vec<String>,
        font_substituted: bool,
        shaping_engine: String,
        letter_spacing: f64,
        ligatures: String,
        font_variations: BTreeMap<String, f64>,
        overflow: bool,
        size: f64,
        weight: String,
        font_style: String,
        align: String,
    },
    Image {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        src: String,
        fit: String,
    },
    Group,
}

fn num(node: &Node, key: &str, default: f64) -> f64 {
    node.derived
        .props
        .get(key)
        .copied()
        .or_else(|| node.authored.props.get(key).and_then(Value::number))
        .unwrap_or(default)
}

fn txt(node: &Node, key: &str) -> Option<String> {
    node.authored
        .props
        .get(key)
        .and_then(Value::text)
        .map(ToOwned::to_owned)
}

fn accessibility(node: &Node) -> DisplayAccessibility {
    let flag = |key: &str| txt(node, key).is_some_and(|value| value == "true");
    DisplayAccessibility {
        label: txt(node, "a11y_label"),
        role: txt(node, "a11y_role"),
        hint: txt(node, "a11y_hint"),
        labelled_by: txt(node, "a11y_labelled_by"),
        live: txt(node, "a11y_live"),
        hidden: flag("a11y_hidden"),
        interactive: flag("interactive"),
        reading_order: node
            .authored
            .props
            .get("reading_order")
            .and_then(Value::number)
            .map(|value| value as u64),
    }
}

fn style(node: &Node, scene: &Scene) -> DisplayStyle {
    let fill_paint = txt(node, "fill_paint").and_then(|name| scene.paints.get(&name).cloned());
    let stroke_paint = txt(node, "stroke_paint").and_then(|name| scene.paints.get(&name).cloned());
    DisplayStyle {
        fill: txt(node, "fill"),
        stroke: txt(node, "stroke"),
        fill_paint,
        stroke_paint,
        stroke_width: num(node, "stroke_width", 1.0),
        opacity: num(node, "opacity", 1.0),
        fill_opacity: num(node, "fill_opacity", 1.0),
        stroke_opacity: num(node, "stroke_opacity", 1.0),
        stroke_linecap: txt(node, "stroke_linecap"),
        stroke_linejoin: txt(node, "stroke_linejoin"),
    }
}

fn transform(node: &Node) -> DisplayTransform {
    let scale = num(node, "scale", 1.0);
    DisplayTransform {
        translate_x: num(node, "translate_x", 0.0),
        translate_y: num(node, "translate_y", 0.0),
        scale_x: num(node, "scale_x", scale),
        scale_y: num(node, "scale_y", scale),
        rotate_deg: num(node, "rotate", num(node, "rotation", 0.0)),
        origin_x: num(node, "origin_x", 0.0),
        origin_y: num(node, "origin_y", 0.0),
    }
}

fn primitive(node: &Node) -> DisplayPrimitive {
    match node.kind {
        NodeKind::Rect => DisplayPrimitive::Rect {
            x: num(node, "x", 0.0),
            y: num(node, "y", 0.0),
            width: num(node, "width", 0.0),
            height: num(node, "height", 0.0),
            rx: num(node, "rx", 0.0),
        },
        NodeKind::Ellipse => DisplayPrimitive::Ellipse {
            cx: num(node, "cx", 0.0),
            cy: num(node, "cy", 0.0),
            rx: num(node, "rx", 0.0),
            ry: num(node, "ry", 0.0),
        },
        NodeKind::Path => DisplayPrimitive::Path {
            d: txt(node, "d").unwrap_or_default(),
        },
        NodeKind::Text => {
            let layout = layout_text_node(node);
            DisplayPrimitive::Text {
                x: num(node, "x", 0.0),
                y: num(node, "y", 0.0),
                lines: layout.lines,
                line_widths: layout.line_widths,
                line_height: layout.line_height,
                ascent: layout.ascent,
                descent: layout.descent,
                leading: layout.leading,
                glyph_count: layout.glyph_count,
                font: if layout.resolved_font.is_empty() {
                    layout.requested_font.clone()
                } else {
                    layout.resolved_font.clone()
                },
                requested_font: layout.requested_font,
                resolved_font: layout.resolved_font,
                postscript_font: layout.postscript_font,
                fallback_fonts: layout.fallback_fonts,
                font_substituted: layout.font_substituted,
                shaping_engine: layout.engine,
                letter_spacing: layout.letter_spacing,
                ligatures: layout.ligatures,
                font_variations: layout.font_variations,
                overflow: layout.overflow,
                size: num(node, "size", 16.0),
                weight: txt(node, "weight").unwrap_or_else(|| "400".into()),
                font_style: txt(node, "font_style").unwrap_or_else(|| "normal".into()),
                align: txt(node, "align").unwrap_or_else(|| "start".into()),
            }
        }
        NodeKind::Image => DisplayPrimitive::Image {
            x: num(node, "x", 0.0),
            y: num(node, "y", 0.0),
            width: num(node, "width", 0.0),
            height: num(node, "height", 0.0),
            src: txt(node, "src").unwrap_or_default(),
            fit: txt(node, "fit").unwrap_or_else(|| "contain".into()),
        },
        NodeKind::Group => DisplayPrimitive::Group,
    }
}

pub fn build_display_list(scene: &Scene) -> DisplayList {
    let definition_ids: BTreeSet<String> = scene
        .nodes
        .iter()
        .flat_map(|node| [txt(node, "clip"), txt(node, "mask")])
        .flatten()
        .collect();
    let mut items = Vec::with_capacity(scene.nodes.len());
    let mut object_map = BTreeMap::new();
    for (index, node) in scene.nodes.iter().enumerate() {
        let bounds = query::bounds_in_scene(scene, node);
        let accessibility = accessibility(node);
        items.push(DisplayItem {
            id: node.id.clone(),
            name: node.name.clone(),
            roles: node.roles.clone(),
            bounds,
            accessibility: accessibility.clone(),
            primitive: primitive(node),
            style: style(node, scene),
            transform: transform(node),
            clip: txt(node, "clip"),
            mask: txt(node, "mask"),
            definition_only: definition_ids.contains(&node.id),
        });
        object_map.insert(
            node.id.clone(),
            ObjectRecord {
                index,
                name: node.name.clone(),
                kind: format!("{:?}", node.kind).to_lowercase(),
                roles: node.roles.clone(),
                bounds,
                accessibility,
                provenance: node.provenance.clone(),
            },
        );
    }
    DisplayList {
        schema_version: 3,
        scene_id: scene.id.clone(),
        revision: scene.revision,
        canvas: DisplayCanvas {
            width: scene.canvas.width,
            height: scene.canvas.height,
            background: scene.canvas.background.clone(),
        },
        items,
        object_map,
    }
}
