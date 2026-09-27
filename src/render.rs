use crate::display_list::{DisplayItem, DisplayPrimitive, build_display_list};
use crate::model::{Paint, Scene};
use anyhow::Result;
use std::collections::BTreeSet;
use std::{fs, path::Path};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fragment_id(prefix: &str, raw: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + raw.len() + 1);
    out.push_str(prefix);
    out.push('-');
    for byte in raw.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("_{byte:02x}"));
        }
    }
    out
}

fn transform_attr(item: &DisplayItem) -> String {
    let t = &item.transform;
    if t.translate_x == 0.0
        && t.translate_y == 0.0
        && t.rotate_deg == 0.0
        && t.scale_x == 1.0
        && t.scale_y == 1.0
    {
        return String::new();
    }
    format!(
        " transform=\"translate({} {}) translate({} {}) rotate({}) scale({} {}) translate({} {})\"",
        t.translate_x,
        t.translate_y,
        t.origin_x,
        t.origin_y,
        t.rotate_deg,
        t.scale_x,
        t.scale_y,
        -t.origin_x,
        -t.origin_y
    )
}

fn common(item: &DisplayItem) -> String {
    let mut attrs = vec![
        format!("id=\"{}\"", esc(&item.id)),
        format!("data-glyph-role=\"{}\"", esc(&item.roles.join(" "))),
    ];
    let transform = transform_attr(item);
    if !transform.is_empty() {
        attrs.push(transform.trim_start().to_string());
    }
    if let Some(clip) = &item.clip {
        attrs.push(format!("clip-path=\"url(#{})\"", fragment_id("clip", clip)));
    }
    if let Some(mask) = &item.mask {
        attrs.push(format!("mask=\"url(#{})\"", fragment_id("mask", mask)));
    }
    attrs.join(" ")
}

fn pct(value: f64) -> String {
    format!("{}%", value * 100.0)
}

fn paint_def(item: &DisplayItem, key: &str, paint: &Paint, defs: &mut Vec<String>) -> String {
    let id = fragment_id(&format!("paint-{key}"), &item.id);
    let stops = |stops: &[crate::model::GradientStop]| {
        stops
            .iter()
            .map(|stop| {
                format!(
                    "<stop offset=\"{}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
                    pct(stop.offset),
                    esc(&stop.color),
                    stop.opacity
                )
            })
            .collect::<String>()
    };
    match paint {
        Paint::Linear {
            x1,
            y1,
            x2,
            y2,
            stops: gradient_stops,
        } => defs.push(format!(
            "<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{}</linearGradient>",
            id,
            pct(*x1),
            pct(*y1),
            pct(*x2),
            pct(*y2),
            stops(gradient_stops)
        )),
        Paint::Radial {
            cx,
            cy,
            r,
            stops: gradient_stops,
        } => defs.push(format!(
            "<radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" r=\"{}\">{}</radialGradient>",
            id,
            pct(*cx),
            pct(*cy),
            pct(*r),
            stops(gradient_stops)
        )),
    }
    format!("url(#{id})")
}

fn style_attrs(item: &DisplayItem, defs: &mut Vec<String>) -> String {
    let style = &item.style;
    let fill = if let Some(paint) = &style.fill_paint {
        paint_def(item, "fill", paint, defs)
    } else {
        style.fill.clone().unwrap_or_else(|| "#000000".into())
    };
    let mut attrs = vec![format!("fill=\"{}\"", esc(&fill))];
    if style.stroke.is_some() || style.stroke_paint.is_some() {
        let stroke = if let Some(paint) = &style.stroke_paint {
            paint_def(item, "stroke", paint, defs)
        } else {
            style.stroke.clone().unwrap_or_else(|| "none".into())
        };
        attrs.push(format!("stroke=\"{}\"", esc(&stroke)));
        attrs.push(format!("stroke-width=\"{}\"", style.stroke_width));
    }
    attrs.push(format!("opacity=\"{}\"", style.opacity));
    attrs.push(format!("fill-opacity=\"{}\"", style.fill_opacity));
    attrs.push(format!("stroke-opacity=\"{}\"", style.stroke_opacity));
    if let Some(cap) = &style.stroke_linecap {
        attrs.push(format!("stroke-linecap=\"{}\"", esc(cap)));
    }
    if let Some(join) = &style.stroke_linejoin {
        attrs.push(format!("stroke-linejoin=\"{}\"", esc(join)));
    }
    attrs.join(" ")
}

fn mask_style_attrs(item: &DisplayItem, defs: &mut Vec<String>) -> String {
    let style = &item.style;
    let fill = if let Some(paint) = &style.fill_paint {
        paint_def(item, "mask-fill", paint, defs)
    } else {
        style.fill.clone().unwrap_or_else(|| "#ffffff".into())
    };
    format!(
        "fill=\"{}\" opacity=\"{}\" fill-opacity=\"{}\"",
        esc(&fill),
        style.opacity,
        style.fill_opacity
    )
}

fn definition_geometry(item: &DisplayItem, mask: bool, defs: &mut Vec<String>) -> String {
    let transform = transform_attr(item);
    let style = if mask {
        format!(" {}", mask_style_attrs(item, defs))
    } else {
        String::new()
    };
    match &item.primitive {
        DisplayPrimitive::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{} />",
            x, y, width, height, rx, transform, style
        ),
        DisplayPrimitive::Ellipse { cx, cy, rx, ry } => format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{}{} />",
            cx, cy, rx, ry, transform, style
        ),
        DisplayPrimitive::Path { d } => format!("<path d=\"{}\"{}{} />", esc(d), transform, style),
        _ => String::new(),
    }
}

fn render_item(item: &DisplayItem, defs: &mut Vec<String>) -> String {
    let style = style_attrs(item, defs);
    match &item.primitive {
        DisplayPrimitive::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => format!(
            "<rect {} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" {}/>\n",
            common(item),
            x,
            y,
            width,
            height,
            rx,
            style
        ),
        DisplayPrimitive::Ellipse { cx, cy, rx, ry } => format!(
            "<ellipse {} cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {}/>\n",
            common(item),
            cx,
            cy,
            rx,
            ry,
            style
        ),
        DisplayPrimitive::Path { d } => {
            format!("<path {} d=\"{}\" {}/>\n", common(item), esc(d), style)
        }
        DisplayPrimitive::Text {
            x,
            y,
            lines,
            line_height,
            font,
            requested_font,
            resolved_font,
            postscript_font,
            fallback_fonts,
            font_substituted,
            shaping_engine,
            letter_spacing,
            ligatures,
            font_variations,
            overflow,
            size,
            weight,
            font_style,
            align,
            ..
        } => {
            let anchor = match align.as_str() {
                "center" => "middle",
                "right" => "end",
                _ => "start",
            };
            let spans = lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    format!(
                        "<tspan x=\"{}\" dy=\"{}\">{}</tspan>",
                        x,
                        if i == 0 { 0.0 } else { *line_height },
                        esc(line)
                    )
                })
                .collect::<String>();
            let feature_settings = match ligatures.as_str() {
                "none" => "'liga' 0, 'clig' 0, 'dlig' 0, 'hlig' 0",
                "all" => "'liga' 1, 'clig' 1, 'dlig' 1, 'hlig' 1",
                _ => "'liga' 1, 'clig' 1",
            };
            let variation_settings = font_variations
                .iter()
                .map(|(tag, value)| format!("'{tag}' {value}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "<text {} x=\"{}\" y=\"{}\" font-size=\"{}\" font-family=\"{}\" font-weight=\"{}\" font-style=\"{}\" letter-spacing=\"{}\" font-feature-settings=\"{}\" font-variation-settings=\"{}\" text-anchor=\"{}\" xml:space=\"preserve\" data-glyph-text-engine=\"{}\" data-glyph-font-requested=\"{}\" data-glyph-font-resolved=\"{}\" data-glyph-font-postscript=\"{}\" data-glyph-font-fallbacks=\"{}\" data-glyph-font-substituted=\"{}\" data-glyph-ligatures=\"{}\" data-glyph-font-variations=\"{}\" data-glyph-text-fit=\"{}\" {}>{}</text>\n",
                common(item),
                x,
                y,
                size,
                esc(font),
                esc(weight),
                esc(font_style),
                letter_spacing,
                feature_settings,
                variation_settings,
                anchor,
                esc(shaping_engine),
                esc(requested_font),
                esc(resolved_font),
                esc(postscript_font),
                esc(&fallback_fonts.join(",")),
                font_substituted,
                esc(ligatures),
                esc(&variation_settings),
                if *overflow { "overflow" } else { "fit" },
                style,
                spans
            )
        }
        DisplayPrimitive::Image {
            x,
            y,
            width,
            height,
            src,
            fit,
        } => {
            let preserve = match fit.as_str() {
                "cover" => "xMidYMid slice",
                "fill" => "none",
                _ => "xMidYMid meet",
            };
            format!(
                "<image {} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" href=\"{}\" preserveAspectRatio=\"{}\" opacity=\"{}\"/>\n",
                common(item),
                x,
                y,
                width,
                height,
                esc(src),
                preserve,
                item.style.opacity
            )
        }
        DisplayPrimitive::Group => format!("<g {}></g>\n", common(item)),
    }
}

pub fn render_svg(scene: &Scene) -> String {
    let display = build_display_list(scene);
    let clip_ids: BTreeSet<&str> = display
        .items
        .iter()
        .filter_map(|item| item.clip.as_deref())
        .collect();
    let mask_ids: BTreeSet<&str> = display
        .items
        .iter()
        .filter_map(|item| item.mask.as_deref())
        .collect();
    let mut defs = Vec::new();
    for item in &display.items {
        if clip_ids.contains(item.id.as_str()) {
            let geometry = definition_geometry(item, false, &mut defs);
            defs.push(format!(
                "<clipPath id=\"{}\">{}</clipPath>",
                fragment_id("clip", &item.id),
                geometry
            ));
        }
        if mask_ids.contains(item.id.as_str()) {
            let geometry = definition_geometry(item, true, &mut defs);
            defs.push(format!(
                "<mask id=\"{}\">{}</mask>",
                fragment_id("mask", &item.id),
                geometry
            ));
        }
    }

    let mut body = String::new();
    if let Some(bg) = &display.canvas.background {
        body.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>\n",
            display.canvas.width,
            display.canvas.height,
            esc(bg)
        ));
    }
    for item in &display.items {
        if !item.definition_only {
            body.push_str(&render_item(item, &mut defs));
        }
    }
    let defs = if defs.is_empty() {
        String::new()
    } else {
        format!("<defs>{}</defs>\n", defs.join(""))
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n{}{}</svg>\n",
        display.canvas.width,
        display.canvas.height,
        display.canvas.width,
        display.canvas.height,
        defs,
        body
    )
}

pub fn write_svg(scene: &Scene, path: impl AsRef<Path>) -> Result<()> {
    fs::write(path, render_svg(scene))?;
    Ok(())
}
