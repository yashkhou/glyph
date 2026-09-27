use crate::{
    display_list::{DisplayCanvas, DisplayItem, DisplayPrimitive, build_display_list},
    model::*,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub node: String,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualDiffReport {
    pub scene_id: String,
    pub before_revision: u64,
    pub after_revision: u64,
    pub canvas_changed: bool,
    pub canvas_before: DisplayCanvas,
    pub canvas_after: DisplayCanvas,
    pub changed_objects: Vec<VisualObjectDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisualObjectDiff {
    pub id: String,
    pub status: String,
    pub name_before: Option<String>,
    pub name_after: Option<String>,
    pub roles_before: Vec<String>,
    pub roles_after: Vec<String>,
    pub bounds_before: Option<[f64; 4]>,
    pub bounds_after: Option<[f64; 4]>,
    pub moved: bool,
    pub resized: bool,
    pub geometry_changed: bool,
    pub style_changed: bool,
    pub content_changed: bool,
    pub semantic_changed: bool,
    pub changed_fields: Vec<String>,
}

fn value_string(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Quantity(q) => format!("{}{}", q.value, q.unit.suffix()),
        Value::Text(s) => s.clone(),
    }
}

pub fn diff_scenes(before: &Scene, after: &Scene) -> Vec<DiffEntry> {
    let a: HashMap<_, _> = before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let b: HashMap<_, _> = after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut ids = BTreeSet::new();
    ids.extend(a.keys().copied());
    ids.extend(b.keys().copied());
    let mut out = Vec::new();
    for id in ids {
        match (a.get(id), b.get(id)) {
            (Some(x), Some(y)) => {
                let mut keys = BTreeSet::new();
                keys.extend(x.authored.props.keys());
                keys.extend(y.authored.props.keys());
                for key in keys {
                    let xv = x.authored.props.get(key).map(value_string);
                    let yv = y.authored.props.get(key).map(value_string);
                    if xv != yv {
                        out.push(DiffEntry {
                            node: id.into(),
                            field: key.clone(),
                            before: xv,
                            after: yv,
                            category: "authored".into(),
                        });
                    }
                }
                if x.roles != y.roles {
                    out.push(DiffEntry {
                        node: id.into(),
                        field: "roles".into(),
                        before: Some(x.roles.join(",")),
                        after: Some(y.roles.join(",")),
                        category: "semantic".into(),
                    });
                }
            }
            (Some(_), None) => out.push(DiffEntry {
                node: id.into(),
                field: "node".into(),
                before: Some("present".into()),
                after: None,
                category: "removed".into(),
            }),
            (None, Some(_)) => out.push(DiffEntry {
                node: id.into(),
                field: "node".into(),
                before: None,
                after: Some("present".into()),
                category: "added".into(),
            }),
            _ => {}
        }
    }
    out
}

fn changed(a: f64, b: f64) -> bool {
    (a - b).abs() > 0.0001
}

fn primitive_moved(a: &DisplayPrimitive, b: &DisplayPrimitive) -> bool {
    match (a, b) {
        (
            DisplayPrimitive::Rect { x: ax, y: ay, .. },
            DisplayPrimitive::Rect { x: bx, y: by, .. },
        ) => changed(*ax, *bx) || changed(*ay, *by),
        (
            DisplayPrimitive::Ellipse { cx: ax, cy: ay, .. },
            DisplayPrimitive::Ellipse { cx: bx, cy: by, .. },
        ) => changed(*ax, *bx) || changed(*ay, *by),
        (
            DisplayPrimitive::Text { x: ax, y: ay, .. },
            DisplayPrimitive::Text { x: bx, y: by, .. },
        ) => changed(*ax, *bx) || changed(*ay, *by),
        _ => false,
    }
}

fn bounds_resized(a: Option<[f64; 4]>, b: Option<[f64; 4]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => changed(a[2], b[2]) || changed(a[3], b[3]),
        (None, None) => false,
        _ => true,
    }
}

fn content_changed(a: &DisplayPrimitive, b: &DisplayPrimitive) -> bool {
    match (a, b) {
        (DisplayPrimitive::Text { lines: a, .. }, DisplayPrimitive::Text { lines: b, .. }) => {
            a != b
        }
        _ => false,
    }
}

fn geometry_changed(a: &DisplayPrimitive, b: &DisplayPrimitive) -> bool {
    match (a, b) {
        (
            DisplayPrimitive::Rect {
                x: ax,
                y: ay,
                width: aw,
                height: ah,
                rx: ar,
            },
            DisplayPrimitive::Rect {
                x: bx,
                y: by,
                width: bw,
                height: bh,
                rx: br,
            },
        ) => [(*ax, *bx), (*ay, *by), (*aw, *bw), (*ah, *bh), (*ar, *br)]
            .iter()
            .any(|(x, y)| changed(*x, *y)),
        (
            DisplayPrimitive::Ellipse {
                cx: ax,
                cy: ay,
                rx: arx,
                ry: ary,
            },
            DisplayPrimitive::Ellipse {
                cx: bx,
                cy: by,
                rx: brx,
                ry: bry,
            },
        ) => [(*ax, *bx), (*ay, *by), (*arx, *brx), (*ary, *bry)]
            .iter()
            .any(|(x, y)| changed(*x, *y)),
        (DisplayPrimitive::Path { d: a }, DisplayPrimitive::Path { d: b }) => a != b,
        (
            DisplayPrimitive::Text {
                x: ax,
                y: ay,
                line_height: al,
                font: af,
                size: asz,
                weight: aw,
                align: aa,
                ..
            },
            DisplayPrimitive::Text {
                x: bx,
                y: by,
                line_height: bl,
                font: bf,
                size: bsz,
                weight: bw,
                align: ba,
                ..
            },
        ) => {
            changed(*ax, *bx)
                || changed(*ay, *by)
                || changed(*al, *bl)
                || changed(*asz, *bsz)
                || af != bf
                || aw != bw
                || aa != ba
        }
        (DisplayPrimitive::Group, DisplayPrimitive::Group) => false,
        _ => true,
    }
}

fn map_items(items: &[DisplayItem]) -> BTreeMap<&str, &DisplayItem> {
    items.iter().map(|i| (i.id.as_str(), i)).collect()
}

pub fn visual_diff(before: &Scene, after: &Scene) -> VisualDiffReport {
    let a = build_display_list(before);
    let b = build_display_list(after);
    let am = map_items(&a.items);
    let bm = map_items(&b.items);
    let mut ids = BTreeSet::new();
    ids.extend(am.keys().copied());
    ids.extend(bm.keys().copied());
    let mut objects = Vec::new();

    for id in ids {
        let (before_item, after_item) = (am.get(id).copied(), bm.get(id).copied());
        let mut fields = Vec::new();
        let (moved, resized, geometry, style, content, semantic, status) =
            match (before_item, after_item) {
                (Some(x), Some(y)) => {
                    let moved = primitive_moved(&x.primitive, &y.primitive);
                    let resized = bounds_resized(x.bounds, y.bounds);
                    let geometry = geometry_changed(&x.primitive, &y.primitive);
                    let style = x.style != y.style;
                    let content = content_changed(&x.primitive, &y.primitive);
                    let semantic = x.roles != y.roles || x.name != y.name;
                    if moved {
                        fields.push("position".into());
                    }
                    if resized {
                        fields.push("size".into());
                    }
                    if geometry && !moved && !resized {
                        fields.push("geometry".into());
                    }
                    if style {
                        fields.push("style".into());
                    }
                    if content {
                        fields.push("content".into());
                    }
                    if x.roles != y.roles {
                        fields.push("roles".into());
                    }
                    if x.name != y.name {
                        fields.push("name".into());
                    }
                    (
                        moved, resized, geometry, style, content, semantic, "changed",
                    )
                }
                (Some(_), None) => {
                    fields.push("removed".into());
                    (false, false, false, false, false, true, "removed")
                }
                (None, Some(_)) => {
                    fields.push("added".into());
                    (false, false, false, false, false, true, "added")
                }
                _ => continue,
            };
        if status == "changed" && fields.is_empty() {
            continue;
        }
        objects.push(VisualObjectDiff {
            id: id.to_string(),
            status: status.into(),
            name_before: before_item.map(|x| x.name.clone()),
            name_after: after_item.map(|x| x.name.clone()),
            roles_before: before_item.map(|x| x.roles.clone()).unwrap_or_default(),
            roles_after: after_item.map(|x| x.roles.clone()).unwrap_or_default(),
            bounds_before: before_item.and_then(|x| x.bounds),
            bounds_after: after_item.and_then(|x| x.bounds),
            moved,
            resized,
            geometry_changed: geometry,
            style_changed: style,
            content_changed: content,
            semantic_changed: semantic,
            changed_fields: fields,
        });
    }

    VisualDiffReport {
        scene_id: after.id.clone(),
        before_revision: before.revision,
        after_revision: after.revision,
        canvas_changed: a.canvas != b.canvas,
        canvas_before: a.canvas,
        canvas_after: b.canvas,
        changed_objects: objects,
    }
}
