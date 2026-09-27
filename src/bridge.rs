use crate::{
    display_list::{DisplayItem, DisplayPrimitive, DisplayTransform, build_display_list},
    model::{Paint, Scene},
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value as JsonValue, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn paint_json(paint: &Paint) -> JsonValue {
    let stop_json = |stops: &[crate::model::GradientStop]| {
        stops
            .iter()
            .map(|stop| {
                json!({
                    "offset": format!("{}%", stop.offset * 100.0),
                    "color": stop.color,
                    "opacity": stop.opacity,
                })
            })
            .collect::<Vec<_>>()
    };
    match paint {
        Paint::Linear {
            x1,
            y1,
            x2,
            y2,
            stops,
        } => json!({
            "type":"linear",
            "x1":format!("{}%",x1*100.0),
            "y1":format!("{}%",y1*100.0),
            "x2":format!("{}%",x2*100.0),
            "y2":format!("{}%",y2*100.0),
            "stops":stop_json(stops),
        }),
        Paint::Radial { cx, cy, r, stops } => json!({
            "type":"radial",
            "cx":format!("{}%",cx*100.0),
            "cy":format!("{}%",cy*100.0),
            "r":format!("{}%",r*100.0),
            "stops":stop_json(stops),
        }),
    }
}

fn transform_matrix(t: &DisplayTransform) -> Option<[f64; 6]> {
    if t.translate_x == 0.0
        && t.translate_y == 0.0
        && t.rotate_deg == 0.0
        && t.scale_x == 1.0
        && t.scale_y == 1.0
    {
        return None;
    }
    let radians = t.rotate_deg.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    let a = cos * t.scale_x;
    let b = sin * t.scale_x;
    let c = -sin * t.scale_y;
    let d = cos * t.scale_y;
    let e = t.translate_x + t.origin_x - a * t.origin_x - c * t.origin_y;
    let f = t.translate_y + t.origin_y - b * t.origin_x - d * t.origin_y;
    Some([a, b, c, d, e, f])
}

fn clip_path(item: &DisplayItem) -> Option<String> {
    match &item.primitive {
        DisplayPrimitive::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => {
            let r = rx.max(0.0).min(width.abs() / 2.0).min(height.abs() / 2.0);
            if r == 0.0 {
                Some(format!("M{x} {y} H{} V{} H{x} Z", x + width, y + height))
            } else {
                Some(format!(
                    "M{} {y} H{} Q{} {y} {} {} V{} Q{} {} {} {} H{} Q{x} {} {x} {} V{} Q{x} {y} {} {y} Z",
                    x + r,
                    x + width - r,
                    x + width,
                    x + width,
                    y + r,
                    y + height - r,
                    x + width,
                    y + height,
                    x + width - r,
                    y + height,
                    x + r,
                    y + height,
                    y + height - r,
                    y + r,
                    x + r,
                ))
            }
        }
        DisplayPrimitive::Ellipse { cx, cy, rx, ry } => Some(format!(
            "M{} {cy} A{rx} {ry} 0 1 0 {} {cy} A{rx} {ry} 0 1 0 {} {cy} Z",
            cx - rx,
            cx + rx,
            cx - rx
        )),
        DisplayPrimitive::Path { d } => Some(d.clone()),
        _ => None,
    }
}

pub fn commander_draw_document(scene: &Scene) -> JsonValue {
    let display = build_display_list(scene);
    let mut warnings = Vec::new();
    let nodes: Vec<JsonValue> = display
        .items
        .iter()
        .filter_map(|item| {
            if item.definition_only || matches!(item.primitive, DisplayPrimitive::Group) {
                return None;
            }
            let mut style = serde_json::Map::new();
            style.insert(
                "fill".into(),
                item.style
                    .fill_paint
                    .as_ref()
                    .map(paint_json)
                    .unwrap_or_else(|| json!(item.style.fill.as_deref().unwrap_or("#000000"))),
            );
            if item.style.stroke.is_some() || item.style.stroke_paint.is_some() {
                style.insert(
                    "stroke".into(),
                    item.style
                        .stroke_paint
                        .as_ref()
                        .map(paint_json)
                        .unwrap_or_else(|| json!(item.style.stroke.as_deref().unwrap_or("none"))),
                );
                style.insert("strokeWidth".into(), json!(item.style.stroke_width));
            }
            style.insert("opacity".into(), json!(item.style.opacity));
            style.insert("fillOpacity".into(), json!(item.style.fill_opacity));
            style.insert("strokeOpacity".into(), json!(item.style.stroke_opacity));
            if let Some(cap) = &item.style.stroke_linecap {
                style.insert("strokeLinecap".into(), json!(cap));
            }
            if let Some(join) = &item.style.stroke_linejoin {
                style.insert("strokeLinejoin".into(), json!(join));
            }
            let style = JsonValue::Object(style);
            let mut base = match &item.primitive {
                DisplayPrimitive::Rect { x, y, width, height, rx } => json!({"id":item.id,"type":"rect","name":item.name,"x":x,"y":y,"width":width,"height":height,"rx":rx,"style":style}),
                DisplayPrimitive::Ellipse { cx, cy, rx, ry } => json!({"id":item.id,"type":"ellipse","name":item.name,"cx":cx,"cy":cy,"rx":rx,"ry":ry,"style":style}),
                DisplayPrimitive::Path { d } => json!({"id":item.id,"type":"path","name":item.name,"d":d,"style":style}),
                DisplayPrimitive::Text { x, y, lines, line_widths, line_height, ascent, descent, leading, glyph_count, font, requested_font, resolved_font, postscript_font, fallback_fonts, font_substituted, shaping_engine, letter_spacing, ligatures, font_variations, overflow, size, weight, font_style, align } => {
                    if *font_substituted {
                        warnings.push(format!(
                            "text {} requested font {} but CoreText resolved {}; exported backends use the resolved font",
                            item.id, requested_font, resolved_font
                        ));
                    }
                    if !fallback_fonts.is_empty() {
                        warnings.push(format!(
                            "text {} uses CoreText fallback fonts {}; CommanderDraw/browser fallback should be visually verified",
                            item.id,
                            fallback_fonts.join(",")
                        ));
                    }
                    if ligatures != "standard" {
                        warnings.push(format!(
                            "text {} uses ligature policy {}; CommanderDraw v1 does not guarantee identical OpenType feature selection",
                            item.id, ligatures
                        ));
                    }
                    if !font_variations.is_empty() {
                        warnings.push(format!(
                            "text {} uses variable-font axes; CommanderDraw v1 preserves the axis metadata but does not guarantee identical variable-font rendering",
                            item.id
                        ));
                    }
                    json!({
                        "id":item.id,
                        "type":"text",
                        "name":item.name,
                        "text":lines.join("\n"),
                        "x":x,
                        "y":y,
                        "fontSize":size,
                        "lineHeight":line_height,
                        "fontFamily":font,
                        "fontWeight":weight,
                        "fontStyle":font_style,
                        "letterSpacing":letter_spacing,
                        "textAnchor":match align.as_str(){"center"=>"middle","right"=>"end",_=>"start"},
                        "style":style,
                        "glyphTextMetrics":{
                            "lineWidths":line_widths,
                            "ascent":ascent,
                            "descent":descent,
                            "leading":leading,
                            "glyphCount":glyph_count,
                            "requestedFont":requested_font,
                            "resolvedFont":resolved_font,
                            "postscriptFont":postscript_font,
                            "fallbackFonts":fallback_fonts,
                            "fontSubstituted":font_substituted,
                            "shapingEngine":shaping_engine,
                            "letterSpacing":letter_spacing,
                            "ligatures":ligatures,
                            "fontVariations":font_variations,
                            "overflow":overflow,
                        }
                    })
                },
                DisplayPrimitive::Image { x, y, width, height, src, fit } => json!({"id":item.id,"type":"image","name":item.name,"x":x,"y":y,"width":width,"height":height,"href":src,"preserveAspectRatio":match fit.as_str(){"cover"=>"xMidYMid slice","fill"=>"none",_=>"xMidYMid meet"},"opacity":item.style.opacity}),
                DisplayPrimitive::Group => unreachable!(),
            };
            let object = base.as_object_mut().expect("CommanderDraw node must be object");
            if item.accessibility != Default::default() {
                object.insert(
                    "glyphAccessibility".into(),
                    json!({
                        "label": item.accessibility.label,
                        "role": item.accessibility.role,
                        "hint": item.accessibility.hint,
                        "labelledBy": item.accessibility.labelled_by,
                        "live": item.accessibility.live,
                        "hidden": item.accessibility.hidden,
                        "interactive": item.accessibility.interactive,
                        "readingOrder": item.accessibility.reading_order,
                    }),
                );
            }
            if let Some(matrix) = transform_matrix(&item.transform) {
                object.insert("transform".into(), json!({"matrix":matrix}));
            }
            if let Some(clip_id) = &item.clip {
                if let Some(target) = display.items.iter().find(|candidate| candidate.id == *clip_id) {
                    if let Some(path) = clip_path(target) {
                        object.insert("clipPath".into(), json!(path));
                        if transform_matrix(&target.transform).is_some() {
                            warnings.push(format!(
                                "clip target {clip_id} has a transform that CommanderDraw clipPath cannot preserve; SVG remains authoritative"
                            ));
                        }
                    }
                }
            }
            if let Some(mask_id) = &item.mask {
                warnings.push(format!(
                    "mask {mask_id} on {} is not representable by CommanderDraw v1; SVG remains authoritative",
                    item.id
                ));
            }
            Some(base)
        })
        .collect();
    json!({
        "schemaVersion":1,
        "id":format!("draw-{}",scene.id),
        "name":scene.name,
        "width":display.canvas.width,
        "height":display.canvas.height,
        "background":display.canvas.background,
        "createdAt":"1970-01-01T00:00:00.000Z",
        "updatedAt":"1970-01-01T00:00:00.000Z",
        "nodes":nodes,
        "meta":{
            "engine":"Glyph display list -> Commander Draw bridge",
            "units":"px",
            "glyphSceneId":scene.id,
            "glyphRevision":scene.revision,
            "glyphDisplayListSchema":display.schema_version,
            "bridgeWarnings":warnings,
        }
    })
}

pub fn write_commander_draw(scene: &Scene, path: impl AsRef<Path>) -> Result<()> {
    fs::write(
        path,
        serde_json::to_string_pretty(&commander_draw_document(scene))? + "\n",
    )?;
    Ok(())
}

fn resolve_node_binary() -> Result<PathBuf> {
    for key in ["GLYPH_NODE_BIN", "NODE_BINARY"] {
        if let Ok(value) = env::var(key) {
            let candidate = PathBuf::from(value);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    if let Some(path) = env::var_os("PATH") {
        for dir in env::split_paths(&path) {
            let candidate = dir.join("node");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    for candidate in [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
        "/usr/bin/node",
    ] {
        let candidate = PathBuf::from(candidate);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(anyhow!(
        "Node.js executable not found; set GLYPH_NODE_BIN or NODE_BINARY"
    ))
}

fn validate_commander_draw_fidelity(scene: &Scene) -> Result<()> {
    let display = build_display_list(scene);
    for item in &display.items {
        if let Some(mask) = &item.mask {
            return Err(anyhow!(
                "CommanderDraw v1 cannot faithfully render Glyph mask {mask} on {}; render SVG or use a mask-capable backend",
                item.id
            ));
        }
        if let Some(clip) = &item.clip
            && let Some(target) = display.items.iter().find(|candidate| candidate.id == *clip)
            && transform_matrix(&target.transform).is_some()
        {
            return Err(anyhow!(
                "CommanderDraw v1 cannot faithfully preserve transformed Glyph clip target {clip}; render SVG or remove the clip transform"
            ));
        }
    }
    Ok(())
}

pub fn render_png_with_commander_draw(
    scene: &Scene,
    output: impl AsRef<Path>,
    commander_root: impl AsRef<Path>,
) -> Result<()> {
    validate_commander_draw_fidelity(scene)?;
    let tmp = std::env::temp_dir().join(format!(
        "glyph-{}-{}.draw.json",
        std::process::id(),
        scene.revision
    ));
    write_commander_draw(scene, &tmp)?;
    let cli = commander_root.as_ref().join("src/cli.mjs");
    if !cli.exists() {
        return Err(anyhow!("CommanderDraw CLI not found at {}", cli.display()));
    }
    let node = resolve_node_binary()?;
    let result = Command::new(&node)
        .arg(cli)
        .arg("render")
        .arg(&tmp)
        .arg(output.as_ref())
        .arg("1")
        .output()
        .with_context(|| format!("failed to invoke CommanderDraw with {}", node.display()))?;
    let _ = fs::remove_file(&tmp);
    if !result.status.success() {
        return Err(anyhow!(
            "CommanderDraw render failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(())
}

pub fn export_pdf_with_commander_draw(
    scene: &Scene,
    output: impl AsRef<Path>,
    commander_root: impl AsRef<Path>,
) -> Result<()> {
    validate_commander_draw_fidelity(scene)?;
    let tmp = std::env::temp_dir().join(format!(
        "glyph-{}-{}.draw.json",
        std::process::id(),
        scene.revision
    ));
    write_commander_draw(scene, &tmp)?;
    let cli = commander_root.as_ref().join("src/cli.mjs");
    if !cli.exists() {
        let _ = fs::remove_file(&tmp);
        return Err(anyhow!("CommanderDraw CLI not found at {}", cli.display()));
    }
    let node = resolve_node_binary()?;
    let options = json!({
        "outputPath": output.as_ref().to_string_lossy(),
        "format": "pdf"
    });
    let result = Command::new(&node)
        .arg(cli)
        .arg("export")
        .arg(&tmp)
        .arg(options.to_string())
        .output()
        .with_context(|| format!("failed to invoke CommanderDraw with {}", node.display()))?;
    let _ = fs::remove_file(&tmp);
    if !result.status.success() {
        return Err(anyhow!(
            "CommanderDraw PDF export failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    let bytes = fs::read(output.as_ref()).with_context(|| {
        format!(
            "CommanderDraw reported PDF export success but {} was not readable",
            output.as_ref().display()
        )
    })?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(anyhow!(
            "CommanderDraw export at {} is not a valid PDF header",
            output.as_ref().display()
        ));
    }
    Ok(())
}
