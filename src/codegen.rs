use crate::{
    conformance::ConformanceStatus,
    display_list::{DisplayItem, DisplayList, DisplayPrimitive, build_display_list},
    model::Scene,
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticCodegenTarget {
    SwiftUi,
    WebCanvas,
}

impl SemanticCodegenTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SwiftUi => "swiftui",
            Self::WebCanvas => "web_canvas",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodegenIssue {
    pub item_id: String,
    pub feature: String,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticCodegenAssessment {
    pub target: SemanticCodegenTarget,
    pub status: ConformanceStatus,
    pub issues: Vec<CodegenIssue>,
    pub warnings: Vec<String>,
}

fn issue(item: &DisplayItem, feature: &str, detail: impl Into<String>) -> CodegenIssue {
    CodegenIssue {
        item_id: item.id.clone(),
        feature: feature.into(),
        detail: detail.into(),
    }
}

fn is_identity_transform(item: &DisplayItem) -> bool {
    let transform = &item.transform;
    transform.translate_x == 0.0
        && transform.translate_y == 0.0
        && transform.scale_x == 1.0
        && transform.scale_y == 1.0
        && transform.rotate_deg == 0.0
        && transform.origin_x == 0.0
        && transform.origin_y == 0.0
}

fn valid_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn validate_color(
    item: &DisplayItem,
    feature: &str,
    value: Option<&str>,
    issues: &mut Vec<CodegenIssue>,
) {
    if let Some(value) = value
        && value != "none"
        && !valid_hex_color(value)
    {
        issues.push(issue(
            item,
            feature,
            format!("semantic codegen only accepts hex colors for now; got {value:?}"),
        ));
    }
}

fn validate_accessibility(
    item: &DisplayItem,
    target: SemanticCodegenTarget,
    issues: &mut Vec<CodegenIssue>,
    warnings: &mut Vec<String>,
) {
    let accessibility = &item.accessibility;
    if accessibility.interactive {
        match target {
            SemanticCodegenTarget::SwiftUi => {
                if accessibility.action.as_deref().is_none_or(str::is_empty) {
                    issues.push(issue(
                        item,
                        "interactive_action",
                        "interactive SwiftUI nodes require an explicit non-empty action contract",
                    ));
                }
            }
            SemanticCodegenTarget::WebCanvas => issues.push(issue(
                item,
                "interactive_accessibility",
                "Canvas codegen has no native interaction contract in v1",
            )),
        }
    } else if accessibility.action.is_some() {
        issues.push(issue(
            item,
            "orphan_action",
            "action is authored but interactive is not true",
        ));
    }
    if accessibility.labelled_by.is_some() || accessibility.live.is_some() {
        issues.push(issue(
            item,
            "accessibility_relationship",
            "label references and live-region behavior require target-specific application wiring",
        ));
    }
    if let Some(role) = accessibility.role.as_deref() {
        match target {
            SemanticCodegenTarget::SwiftUi if matches!(role, "text" | "image" | "header" | "button") => {}
            SemanticCodegenTarget::WebCanvas => warnings.push(format!(
                "{}: accessibility role {role:?} is preserved in generated metadata, not a native Canvas semantic node",
                item.id
            )),
            _ => issues.push(issue(
                item,
                "accessibility_role",
                format!("accessibility role {role:?} has no safe target mapping in this emitter"),
            )),
        }
    }
    if target == SemanticCodegenTarget::WebCanvas
        && (accessibility.label.is_some()
            || accessibility.hint.is_some()
            || accessibility.hidden
            || accessibility.reading_order.is_some())
    {
        warnings.push(format!(
            "{}: object accessibility metadata is exported in glyphObjects; Canvas itself has no per-object accessibility tree",
            item.id
        ));
    }
}

fn validate_item(
    item: &DisplayItem,
    target: SemanticCodegenTarget,
    issues: &mut Vec<CodegenIssue>,
    warnings: &mut Vec<String>,
) {
    if item.definition_only {
        issues.push(issue(
            item,
            "definition_only",
            "clip/mask definition nodes are outside the semantic codegen subset",
        ));
    }
    if item.clip.is_some() || item.mask.is_some() {
        issues.push(issue(
            item,
            "clip_or_mask",
            "clip and mask semantics are not in codegen subset v1",
        ));
    }
    if item.style.fill_paint.is_some() || item.style.stroke_paint.is_some() {
        issues.push(issue(
            item,
            "paint",
            "typed gradients/paints are not in codegen subset v1",
        ));
    }
    if item.style.stroke_linecap.is_some() || item.style.stroke_linejoin.is_some() {
        issues.push(issue(
            item,
            "stroke_geometry",
            "explicit line-cap/join semantics are not in codegen subset v1",
        ));
    }
    if !is_identity_transform(item) {
        issues.push(issue(item, "transform", "affine transforms are not in semantic codegen subset v1; bake or restructure intent first"));
    }
    validate_color(item, "fill", item.style.fill.as_deref(), issues);
    validate_color(item, "stroke", item.style.stroke.as_deref(), issues);
    validate_accessibility(item, target, issues, warnings);

    match &item.primitive {
        DisplayPrimitive::Rect { .. }
        | DisplayPrimitive::Ellipse { .. }
        | DisplayPrimitive::Group => {}
        DisplayPrimitive::Text {
            ligatures,
            font_variations,
            letter_spacing,
            ..
        } => {
            if ligatures != "standard" {
                issues.push(issue(
                    item,
                    "ligatures",
                    "non-standard ligature policy has no stable codegen mapping",
                ));
            }
            if !font_variations.is_empty() {
                issues.push(issue(
                    item,
                    "variable_fonts",
                    "variable-font axes are outside codegen subset v1",
                ));
            }
            if target == SemanticCodegenTarget::WebCanvas && letter_spacing.abs() > f64::EPSILON {
                issues.push(issue(
                    item,
                    "letter_spacing",
                    "Canvas letter-spacing support is not portable enough for codegen v1",
                ));
            }
            if item.style.stroke.is_some() {
                issues.push(issue(
                    item,
                    "text_stroke",
                    "outlined text is outside codegen subset v1",
                ));
            }
            warnings.push(format!(
                "{}: generated text keeps Glyph line breaks and frame geometry, but target font shaping may differ from CoreText",
                item.id
            ));
        }
        DisplayPrimitive::Path { .. } => issues.push(issue(
            item,
            "path",
            "arbitrary path geometry is deliberately excluded from readable semantic codegen v1",
        )),
        DisplayPrimitive::Image { src, fit, .. } => match target {
            SemanticCodegenTarget::SwiftUi => {
                let Some(asset_name) = src.strip_prefix("asset:") else {
                    issues.push(issue(
                        item,
                        "image",
                        "SwiftUI image codegen requires explicit asset:<catalog-name> ownership",
                    ));
                    return;
                };
                if asset_name.is_empty() {
                    issues.push(issue(item, "image", "asset: image name cannot be empty"));
                }
                if !matches!(fit.as_str(), "contain" | "fill" | "cover") {
                    issues.push(issue(
                        item,
                        "image_fit",
                        format!("unsupported SwiftUI image fit {fit:?}; expected contain, fill, or cover"),
                    ));
                }
            }
            SemanticCodegenTarget::WebCanvas => issues.push(issue(
                item,
                "image",
                "Web Canvas image ownership/bundling is not in codegen subset v1",
            )),
        },
    }
}

pub fn assess_semantic_codegen(
    scene: &Scene,
    target: SemanticCodegenTarget,
) -> SemanticCodegenAssessment {
    let display = build_display_list(scene);
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    if let Some(background) = display.canvas.background.as_deref()
        && background != "none"
        && !valid_hex_color(background)
    {
        issues.push(CodegenIssue {
            item_id: scene.id.clone(),
            feature: "canvas_background".into(),
            detail: format!("semantic codegen only accepts hex canvas colors; got {background:?}"),
        });
    }
    for item in &display.items {
        validate_item(item, target, &mut issues, &mut warnings);
    }
    warnings.sort();
    warnings.dedup();
    let status = if !issues.is_empty() {
        ConformanceStatus::Unsupported
    } else if !warnings.is_empty() {
        ConformanceStatus::Degraded
    } else {
        ConformanceStatus::Exact
    };
    SemanticCodegenAssessment {
        target,
        status,
        issues,
        warnings,
    }
}

fn checked_display(scene: &Scene, target: SemanticCodegenTarget) -> Result<DisplayList> {
    let assessment = assess_semantic_codegen(scene, target);
    if assessment.status == ConformanceStatus::Unsupported {
        let details = assessment
            .issues
            .iter()
            .map(|issue| format!("{}.{}: {}", issue.item_id, issue.feature, issue.detail))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow!(
            "{} semantic codegen is unsupported for this scene: {details}",
            target.as_str()
        ));
    }
    Ok(build_display_list(scene))
}

fn swift_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn js_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn swift_color(value: Option<&str>, opacity: f64) -> String {
    match value {
        Some("none") | None => "Color.clear".into(),
        Some(value) => format!(
            "Color(glyphHex: \"{}\").opacity({opacity})",
            swift_escape(value)
        ),
    }
}

fn swift_accessibility(item: &DisplayItem) -> String {
    let mut out = format!(
        "\n                .accessibilityIdentifier(\"{}\")",
        swift_escape(&item.id)
    );
    if let Some(label) = &item.accessibility.label {
        out.push_str(&format!(
            "\n                .accessibilityLabel(Text(\"{}\"))",
            swift_escape(label)
        ));
    }
    if let Some(hint) = &item.accessibility.hint {
        out.push_str(&format!(
            "\n                .accessibilityHint(Text(\"{}\"))",
            swift_escape(hint)
        ));
    }
    if item.accessibility.hidden {
        out.push_str("\n                .accessibilityHidden(true)");
    }
    if let Some(order) = item.accessibility.reading_order {
        out.push_str(&format!(
            "\n                .accessibilitySortPriority({})",
            -(order as i64)
        ));
    }
    if let Some(role) = item.accessibility.role.as_deref() {
        match role {
            "image" => out.push_str("\n                .accessibilityAddTraits(.isImage)"),
            "header" => out.push_str("\n                .accessibilityAddTraits(.isHeader)"),
            _ => {}
        }
    }
    out
}

fn swift_visual_accessibility(item: &DisplayItem) -> String {
    if item.accessibility.interactive {
        "\n                .accessibilityHidden(true)".into()
    } else {
        swift_accessibility(item)
    }
}

fn swift_interaction_overlay(item: &DisplayItem) -> Option<String> {
    if !item.accessibility.interactive {
        return None;
    }
    let action = item.accessibility.action.as_deref()?;
    let bounds = item.bounds?;
    let label = item
        .accessibility
        .label
        .as_deref()
        .unwrap_or(item.name.as_str());
    let mut out = format!(
        "                Button(action: {{ performAction(\"{}\") }}) {{\n                    Color.clear\n                        .frame(width: {}, height: {})\n                        .contentShape(Rectangle())\n                }}\n                .buttonStyle(.plain)\n                .position(x: {}, y: {})\n                .accessibilityIdentifier(\"{}\")\n                .accessibilityLabel(Text(\"{}\"))",
        swift_escape(action),
        bounds[2],
        bounds[3],
        bounds[0] + bounds[2] / 2.0,
        bounds[1] + bounds[3] / 2.0,
        swift_escape(&item.id),
        swift_escape(label)
    );
    if let Some(hint) = &item.accessibility.hint {
        out.push_str(&format!(
            "\n                .accessibilityHint(Text(\"{}\"))",
            swift_escape(hint)
        ));
    }
    if let Some(order) = item.accessibility.reading_order {
        out.push_str(&format!(
            "\n                .accessibilitySortPriority({})",
            -(order as i64)
        ));
    }
    Some(out)
}

fn swift_shape_style(item: &DisplayItem, shape: &str) -> String {
    let mut out = format!(
        "                {shape}\n                    .fill({})",
        swift_color(item.style.fill.as_deref(), item.style.fill_opacity)
    );
    if let Some(stroke) = item.style.stroke.as_deref()
        && stroke != "none"
    {
        out.push_str(&format!(
            "\n                    .overlay({shape}.stroke({}, lineWidth: {}))",
            swift_color(Some(stroke), item.style.stroke_opacity),
            item.style.stroke_width
        ));
    }
    out
}
fn render_swift_item(item: &DisplayItem) -> String {
    let accessibility = swift_visual_accessibility(item);
    match &item.primitive {
        DisplayPrimitive::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => {
            let shape = format!("RoundedRectangle(cornerRadius: {rx})");
            format!(
                "{}\n                    .frame(width: {width}, height: {height})\n                    .position(x: {}, y: {})\n                    .opacity({}){}",
                swift_shape_style(item, &shape),
                x + width / 2.0,
                y + height / 2.0,
                item.style.opacity,
                accessibility
            )
        }
        DisplayPrimitive::Ellipse { cx, cy, rx, ry } => format!(
            "{}\n                    .frame(width: {}, height: {})\n                    .position(x: {cx}, y: {cy})\n                    .opacity({}){}",
            swift_shape_style(item, "Ellipse()"),
            rx * 2.0,
            ry * 2.0,
            item.style.opacity,
            accessibility
        ),
        DisplayPrimitive::Text {
            lines,
            requested_font,
            postscript_font,
            font,
            size,
            weight,
            letter_spacing,
            align,
            ..
        } => {
            let bounds = item.bounds.unwrap_or([0.0, 0.0, 0.0, 0.0]);
            let text = swift_escape(&lines.join("\n"));
            let font_name = if postscript_font.is_empty() {
                font
            } else {
                postscript_font
            };
            let alignment = match align.as_str() {
                "center" => "center",
                "right" => "trailing",
                _ => "leading",
            };
            let swift_weight = match weight.as_str() {
                "900" => "black",
                "800" => "heavy",
                "700" => "bold",
                "600" => "semibold",
                "500" => "medium",
                "300" => "light",
                "200" => "ultraLight",
                "100" => "thin",
                _ => "regular",
            };
            let font_modifier = match requested_font.as_str() {
                "system-rounded" => format!(
                    ".font(.system(size: {size}, weight: .{swift_weight}, design: .rounded))"
                ),
                "system" | "system-ui" => {
                    format!(".font(.system(size: {size}, weight: .{swift_weight}))")
                }
                _ => format!(
                    ".font(.custom(\"{}\", size: {size}))",
                    swift_escape(font_name)
                ),
            };
            format!(
                "                Text(\"{text}\")\n                    {font_modifier}\n                    .tracking({letter_spacing})\n                    .foregroundStyle({})\n                    .multilineTextAlignment(.{alignment})\n                    .frame(width: {}, height: {}, alignment: .{alignment})\n                    .position(x: {}, y: {})\n                    .opacity({}){}",
                swift_color(
                    item.style.fill.as_deref().or(Some("#000000")),
                    item.style.fill_opacity
                ),
                bounds[2],
                bounds[3],
                bounds[0] + bounds[2] / 2.0,
                bounds[1] + bounds[3] / 2.0,
                item.style.opacity,
                accessibility
            )
        }
        DisplayPrimitive::Group => {
            let bounds = item.bounds.unwrap_or([0.0, 0.0, 0.0, 0.0]);
            format!(
                "                Color.clear\n                    .frame(width: {}, height: {})\n                    .position(x: {}, y: {})\n                    .allowsHitTesting(false){}",
                bounds[2],
                bounds[3],
                bounds[0] + bounds[2] / 2.0,
                bounds[1] + bounds[3] / 2.0,
                accessibility
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
            let asset_name = src
                .strip_prefix("asset:")
                .expect("validated SwiftUI image asset ownership");
            let scaling = if fit == "contain" {
                ".scaledToFit()"
            } else {
                ".scaledToFill()"
            };
            let clip = if fit == "contain" {
                ""
            } else {
                "\n                    .clipped()"
            };
            format!(
                "                Image(\"{}\")\n                    .resizable()\n                    .renderingMode(.original)\n                    {}\n                    .frame(width: {width}, height: {height}){}\n                    .position(x: {}, y: {})\n                    .opacity({}){}",
                swift_escape(asset_name),
                scaling,
                clip,
                x + width / 2.0,
                y + height / 2.0,
                item.style.opacity,
                accessibility
            )
        }
        DisplayPrimitive::Path { .. } => unreachable!("validated codegen subset"),
    }
}

pub fn render_swiftui(scene: &Scene) -> Result<String> {
    let display = checked_display(scene, SemanticCodegenTarget::SwiftUi)?;
    let mut out = String::from("import SwiftUI\n\n");
    out.push_str("public struct GlyphSceneView: View {\n    private let performAction: (String) -> Void\n\n    public init(performAction: @escaping (String) -> Void = { _ in }) {\n        self.performAction = performAction\n    }\n\n    public var body: some View {\n        ZStack(alignment: .topLeading) {\n");
    for item in &display.items {
        out.push_str(&render_swift_item(item));
        out.push('\n');
        if let Some(overlay) = swift_interaction_overlay(item) {
            out.push_str(&overlay);
            out.push('\n');
        }
    }
    out.push_str("        }\n");
    out.push_str(&format!(
        "        .frame(width: {}, height: {}, alignment: .topLeading)\n",
        display.canvas.width, display.canvas.height
    ));
    if let Some(background) = display.canvas.background.as_deref()
        && background != "none"
    {
        out.push_str(&format!(
            "        .background(Color(glyphHex: \"{}\"))\n",
            swift_escape(background)
        ));
    }
    out.push_str("        .clipped()\n    }\n}\n\n");
    out.push_str("private extension Color {\n    init(glyphHex: String) {\n        let hex = glyphHex.dropFirst()\n        let expanded: String\n        if hex.count == 3 || hex.count == 4 {\n            expanded = hex.map { \"\\($0)\\($0)\" }.joined()\n        } else {\n            expanded = String(hex)\n        }\n        let value = UInt64(expanded, radix: 16) ?? 0\n");
    out.push_str("        let hasAlpha = expanded.count == 8\n        let red = Double((value >> (hasAlpha ? 24 : 16)) & 0xff) / 255\n        let green = Double((value >> (hasAlpha ? 16 : 8)) & 0xff) / 255\n        let blue = Double((value >> (hasAlpha ? 8 : 0)) & 0xff) / 255\n        let alpha = hasAlpha ? Double(value & 0xff) / 255 : 1\n        self.init(.sRGB, red: red, green: green, blue: blue, opacity: alpha)\n    }\n}\n");
    Ok(out)
}
fn web_fill(item: &DisplayItem, out: &mut String) {
    if let Some(fill) = item.style.fill.as_deref()
        && fill != "none"
    {
        out.push_str(&format!(
            "  ctx.globalAlpha = {}; ctx.fillStyle = \"{}\"; ctx.fill();\n",
            item.style.opacity * item.style.fill_opacity,
            js_escape(fill)
        ));
    }
    if let Some(stroke) = item.style.stroke.as_deref()
        && stroke != "none"
    {
        out.push_str(&format!(
            "  ctx.globalAlpha = {}; ctx.strokeStyle = \"{}\"; ctx.lineWidth = {}; ctx.stroke();\n",
            item.style.opacity * item.style.stroke_opacity,
            js_escape(stroke),
            item.style.stroke_width
        ));
    }
}

fn render_web_item(item: &DisplayItem, out: &mut String) {
    match &item.primitive {
        DisplayPrimitive::Rect {
            x,
            y,
            width,
            height,
            rx,
        } => {
            out.push_str(&format!("  // Glyph {}\n  ctx.save(); ctx.beginPath(); glyphRoundRect(ctx, {x}, {y}, {width}, {height}, {rx});\n", js_escape(&item.id)));
            web_fill(item, out);
            out.push_str("  ctx.restore();\n");
        }
        DisplayPrimitive::Ellipse { cx, cy, rx, ry } => {
            out.push_str(&format!("  // Glyph {}\n  ctx.save(); ctx.beginPath(); ctx.ellipse({cx}, {cy}, {rx}, {ry}, 0, 0, Math.PI * 2);\n", js_escape(&item.id)));
            web_fill(item, out);
            out.push_str("  ctx.restore();\n");
        }
        DisplayPrimitive::Text {
            x,
            y,
            lines,
            line_height,
            postscript_font,
            font,
            size,
            weight,
            font_style,
            align,
            ..
        } => {
            let font_name = if postscript_font.is_empty() {
                font
            } else {
                postscript_font
            };
            let text_align = match align.as_str() {
                "center" => "center",
                "right" => "right",
                _ => "left",
            };
            let fill = item.style.fill.as_deref().unwrap_or("#000000");
            out.push_str(&format!(
                "  // Glyph {}\n  ctx.save(); ctx.globalAlpha = {}; ctx.fillStyle = \"{}\"; ctx.font = \"{} {} {}px \\\"{}\\\"\"; ctx.textAlign = \"{}\"; ctx.textBaseline = \"alphabetic\";\n",
                js_escape(&item.id),
                item.style.opacity * item.style.fill_opacity,
                js_escape(fill),
                js_escape(font_style),
                js_escape(weight),
                size,
                js_escape(font_name),
                text_align
            ));
            for (index, line) in lines.iter().enumerate() {
                out.push_str(&format!(
                    "  ctx.fillText(`{}`, {x}, {});\n",
                    js_escape(line),
                    y + *line_height * index as f64
                ));
            }
            out.push_str("  ctx.restore();\n");
        }
        DisplayPrimitive::Group => {
            out.push_str(&format!(
                "  // Glyph group {} (non-rendering structural identity)\n",
                js_escape(&item.id)
            ));
        }
        DisplayPrimitive::Path { .. } | DisplayPrimitive::Image { .. } => {
            unreachable!("validated codegen subset")
        }
    }
}

pub fn render_web_canvas(scene: &Scene) -> Result<String> {
    let display = checked_display(scene, SemanticCodegenTarget::WebCanvas)?;
    let object_json = serde_json::to_string_pretty(&display.object_map)?;
    let mut out = format!(
        "export const glyphScene = {{ id: \"{}\", revision: {}, width: {}, height: {} }};\n\nexport const glyphObjects = {};\n\n",
        js_escape(&display.scene_id),
        display.revision,
        display.canvas.width,
        display.canvas.height,
        object_json
    );
    out.push_str("function glyphRoundRect(ctx, x, y, width, height, radius) {\n  const r = Math.max(0, Math.min(radius, width / 2, height / 2));\n  ctx.moveTo(x + r, y);\n  ctx.lineTo(x + width - r, y);\n  ctx.quadraticCurveTo(x + width, y, x + width, y + r);\n  ctx.lineTo(x + width, y + height - r);\n  ctx.quadraticCurveTo(x + width, y + height, x + width - r, y + height);\n  ctx.lineTo(x + r, y + height);\n  ctx.quadraticCurveTo(x, y + height, x, y + height - r);\n  ctx.lineTo(x, y + r);\n  ctx.quadraticCurveTo(x, y, x + r, y);\n  ctx.closePath();\n}\n\n");
    out.push_str("export function renderGlyph(canvas) {\n  if (!(canvas instanceof HTMLCanvasElement)) throw new TypeError(\"renderGlyph expects an HTMLCanvasElement\");\n  canvas.width = glyphScene.width; canvas.height = glyphScene.height;\n  const ctx = canvas.getContext(\"2d\");\n  if (!ctx) throw new Error(\"2D canvas context unavailable\");\n  ctx.clearRect(0, 0, canvas.width, canvas.height);\n");
    if let Some(background) = display.canvas.background.as_deref()
        && background != "none"
    {
        out.push_str(&format!(
            "  ctx.save(); ctx.globalAlpha = 1; ctx.fillStyle = \"{}\"; ctx.fillRect(0, 0, canvas.width, canvas.height); ctx.restore();\n",
            js_escape(background)
        ));
    }
    for item in &display.items {
        render_web_item(item, &mut out);
    }
    out.push_str(
        "  ctx.globalAlpha = 1;\n  return { scene: glyphScene, objects: glyphObjects };\n}\n",
    );
    Ok(out)
}
