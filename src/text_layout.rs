use crate::model::{Node, NodeKind, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub lines: Vec<String>,
    pub line_widths: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub line_height: f64,
    pub ascent: f64,
    pub descent: f64,
    pub leading: f64,
    pub glyph_count: usize,
    pub engine: String,
    pub requested_font: String,
    pub resolved_font: String,
    pub postscript_font: String,
    pub fallback_fonts: Vec<String>,
    pub font_substituted: bool,
    pub letter_spacing: f64,
    pub ligatures: String,
    pub font_variations: BTreeMap<String, f64>,
    pub overflow: bool,
}

fn authored_number(node: &Node, key: &str) -> Option<f64> {
    node.authored.props.get(key).and_then(Value::number)
}

fn authored_text<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    node.authored.props.get(key).and_then(Value::text)
}

fn utf16_to_byte(text: &str, target: usize) -> usize {
    if target == 0 {
        return 0;
    }
    let mut utf16 = 0usize;
    for (byte, ch) in text.char_indices() {
        if utf16 == target {
            return byte;
        }
        utf16 += ch.len_utf16();
        if utf16 > target {
            return byte;
        }
    }
    text.len()
}

fn utf16_slice(text: &str, start: usize, length: usize) -> String {
    let a = utf16_to_byte(text, start);
    let b = utf16_to_byte(text, start.saturating_add(length));
    text[a..b].to_string()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use core_foundation::attributed_string::{CFAttributedStringRef, CFMutableAttributedString};
    use core_foundation::base::{CFIndex, CFRange, CFRelease, CFTypeRef, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::{CFString, CFStringRef};
    use core_graphics::base::CGFloat;
    use core_text::font::{self, CTFont, kCTFontEmphasizedSystemFontType, kCTFontSystemFontType};
    use core_text::font_descriptor::{self, kCTFontBoldTrait, kCTFontItalicTrait};
    use core_text::line::{CTLine, CTLineRef};
    use core_text::string_attributes::{
        kCTFontAttributeName, kCTKernAttributeName, kCTLigatureAttributeName,
    };
    use std::ffi::c_void;

    #[repr(C)]
    struct __CTTypesetter(c_void);
    type CTTypesetterRef = *const __CTTypesetter;

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTTypesetterCreateWithAttributedString(string: CFAttributedStringRef)
        -> CTTypesetterRef;
        fn CTTypesetterSuggestLineBreak(
            typesetter: CTTypesetterRef,
            start_index: CFIndex,
            width: f64,
        ) -> CFIndex;
        fn CTTypesetterSuggestClusterBreak(
            typesetter: CTTypesetterRef,
            start_index: CFIndex,
            width: f64,
        ) -> CFIndex;
        fn CTTypesetterCreateLine(typesetter: CTTypesetterRef, range: CFRange) -> CTLineRef;
        fn CTLineGetTrailingWhitespaceWidth(line: CTLineRef) -> CGFloat;
        static kCTFontVariationAxisIdentifierKey: CFStringRef;
        static kCTFontVariationAxisMinimumValueKey: CFStringRef;
        static kCTFontVariationAxisMaximumValueKey: CFStringRef;
    }

    struct Typesetter(CTTypesetterRef);

    impl Drop for Typesetter {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0.cast::<c_void>() as CFTypeRef) };
            }
        }
    }

    fn requested_weight(node: &Node) -> u16 {
        authored_text(node, "weight")
            .and_then(|raw| raw.parse::<u16>().ok())
            .unwrap_or(400)
            .clamp(1, 1000)
    }

    fn ligature_mode(node: &Node) -> Result<(&str, i32), String> {
        match authored_text(node, "ligatures").unwrap_or("standard") {
            "none" => Ok(("none", 0)),
            "standard" => Ok(("standard", 1)),
            "all" => Ok(("all", 2)),
            other => Err(format!(
                "unsupported ligatures mode {other:?}; expected none, standard, or all"
            )),
        }
    }

    fn normalize_font_name(raw: &str) -> String {
        raw.chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    }

    fn requested_variations(node: &Node) -> BTreeMap<String, f64> {
        node.authored
            .props
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("font_axis_")
                    .and_then(|tag| value.number().map(|number| (tag.to_string(), number)))
            })
            .collect()
    }

    fn axis_tag(identifier: i64) -> String {
        let bytes = (identifier as u32).to_be_bytes();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn apply_variations(font: CTFont, requested: &BTreeMap<String, f64>) -> Result<CTFont, String> {
        if requested.is_empty() {
            return Ok(font);
        }
        let axes = font.get_variation_axes().ok_or_else(|| {
            format!(
                "font {} does not expose variable-font axes",
                font.postscript_name()
            )
        })?;
        let id_key = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisIdentifierKey) };
        let min_key = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisMinimumValueKey) };
        let max_key = unsafe { CFString::wrap_under_get_rule(kCTFontVariationAxisMaximumValueKey) };
        let mut available = BTreeMap::new();
        for axis in axes.iter() {
            let Some(identifier) = axis
                .find(id_key.clone())
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_i64())
            else {
                continue;
            };
            let Some(minimum) = axis
                .find(min_key.clone())
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_f64())
            else {
                continue;
            };
            let Some(maximum) = axis
                .find(max_key.clone())
                .and_then(|value| value.downcast::<CFNumber>())
                .and_then(|value| value.to_f64())
            else {
                continue;
            };
            available.insert(axis_tag(identifier), (identifier, minimum, maximum));
        }
        let mut values = Vec::new();
        for (tag, value) in requested {
            let Some((identifier, minimum, maximum)) = available.get(tag).copied() else {
                return Err(format!(
                    "font {} does not expose requested variation axis {tag}",
                    font.postscript_name()
                ));
            };
            if *value < minimum || *value > maximum {
                return Err(format!(
                    "font variation axis {tag}={} is outside supported range [{minimum}, {maximum}] for {}",
                    value,
                    font.postscript_name()
                ));
            }
            values.push((CFNumber::from(identifier), CFNumber::from(*value)));
        }
        let values = CFDictionary::from_CFType_pairs(&values);
        let variation_attribute =
            unsafe { CFString::wrap_under_get_rule(font_descriptor::kCTFontVariationAttribute) };
        let attributes = CFDictionary::from_CFType_pairs(&[(variation_attribute, values)]);
        let descriptor = font
            .copy_descriptor()
            .create_copy_with_attributes(attributes.to_untyped())
            .map_err(|_| {
                format!(
                    "CoreText failed to create variable-font descriptor for {}",
                    font.postscript_name()
                )
            })?;
        Ok(font::new_from_descriptor(&descriptor, font.pt_size()))
    }

    fn make_font(node: &Node, size: f64) -> Result<(CTFont, BTreeMap<String, f64>), String> {
        let requested = authored_text(node, "font").unwrap_or("system-ui");
        let weight = requested_weight(node);
        let mut font = if requested == "system-ui" || requested == "system" {
            font::new_ui_font_for_language(
                if weight >= 600 {
                    kCTFontEmphasizedSystemFontType
                } else {
                    kCTFontSystemFontType
                },
                size,
                None,
            )
        } else {
            font::new_from_name(requested, size)
                .map_err(|_| format!("CoreText could not resolve font {requested:?}"))?
        };
        let variations = requested_variations(node);
        let mut traits = 0u32;
        if weight >= 600
            && requested != "system-ui"
            && requested != "system"
            && !variations.contains_key("wght")
        {
            traits |= kCTFontBoldTrait;
        }
        if authored_text(node, "font_style") == Some("italic")
            && !variations.contains_key("ital")
            && !variations.contains_key("slnt")
        {
            traits |= kCTFontItalicTrait;
        }
        if traits != 0 {
            if let Some(styled) = font.clone_with_symbolic_traits(traits, traits) {
                font = styled;
            }
        }
        font = apply_variations(font, &variations)?;
        Ok((font, variations))
    }

    fn attributed(
        text: &str,
        font: &CTFont,
        letter_spacing: f64,
        ligature_value: i32,
    ) -> CFMutableAttributedString {
        let mut string = CFMutableAttributedString::new();
        string.replace_str(&CFString::new(text), CFRange::init(0, 0));
        let len = string.char_len();
        let kern = CFNumber::from(letter_spacing);
        let ligatures = CFNumber::from(ligature_value);
        unsafe {
            string.set_attribute(CFRange::init(0, len), kCTFontAttributeName, font);
            string.set_attribute(CFRange::init(0, len), kCTKernAttributeName, &kern);
            string.set_attribute(CFRange::init(0, len), kCTLigatureAttributeName, &ligatures);
        }
        string
    }

    fn typesetter(string: &CFMutableAttributedString) -> Result<Typesetter, String> {
        let raw = unsafe { CTTypesetterCreateWithAttributedString(string.as_concrete_TypeRef()) };
        if raw.is_null() {
            Err("CTTypesetterCreateWithAttributedString returned null".into())
        } else {
            Ok(Typesetter(raw))
        }
    }

    fn run_fallbacks(line: &CTLine, primary: &str, fallbacks: &mut BTreeSet<String>) {
        for run in line.glyph_runs().iter() {
            let Some(attributes) = run.attributes() else {
                continue;
            };
            let Some(value) = attributes.find(CFString::new("NSFont")) else {
                continue;
            };
            let Some(font) = value.downcast::<CTFont>() else {
                continue;
            };
            let name = font.postscript_name();
            if name != primary {
                fallbacks.insert(name);
            }
        }
    }

    fn layout_paragraph(
        paragraph: &str,
        font: &CTFont,
        primary_font: &str,
        max_width: Option<f64>,
        letter_spacing: f64,
        ligature_value: i32,
        lines: &mut Vec<String>,
        widths: &mut Vec<f64>,
        ascent: &mut f64,
        descent: &mut f64,
        leading: &mut f64,
        glyph_count: &mut usize,
        fallbacks: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if paragraph.is_empty() {
            lines.push(String::new());
            widths.push(0.0);
            *ascent = ascent.max(font.ascent());
            *descent = descent.max(font.descent());
            *leading = leading.max(font.leading());
            return Ok(());
        }
        let attributed = attributed(paragraph, font, letter_spacing, ligature_value);
        let setter = typesetter(&attributed)?;
        let total = usize::try_from(attributed.char_len()).unwrap_or(0);
        let mut start = 0usize;
        while start < total {
            let remaining = total - start;
            let count = if let Some(limit) = max_width {
                let mut count = usize::try_from(unsafe {
                    CTTypesetterSuggestLineBreak(setter.0, start as CFIndex, limit)
                })
                .unwrap_or(0)
                .min(remaining);
                if count == 0 {
                    count = usize::try_from(unsafe {
                        CTTypesetterSuggestClusterBreak(setter.0, start as CFIndex, limit)
                    })
                    .unwrap_or(0)
                    .min(remaining);
                }
                if count == 0 {
                    let rest = utf16_slice(paragraph, start, remaining);
                    count = rest.chars().next().map(char::len_utf16).unwrap_or(1);
                }
                count
            } else {
                remaining
            };
            let raw_line = unsafe {
                CTTypesetterCreateLine(setter.0, CFRange::init(start as CFIndex, count as CFIndex))
            };
            if raw_line.is_null() {
                return Err("CTTypesetterCreateLine returned null".into());
            }
            let line = unsafe { CTLine::wrap_under_create_rule(raw_line) };
            let metrics = line.get_typographic_bounds();
            let whitespace =
                unsafe { CTLineGetTrailingWhitespaceWidth(line.as_concrete_TypeRef()) };
            lines.push(utf16_slice(paragraph, start, count));
            widths.push((metrics.width - whitespace).max(0.0));
            *ascent = ascent.max(metrics.ascent.max(font.ascent()));
            *descent = descent.max(metrics.descent.max(font.descent()));
            *leading = leading.max(metrics.leading.max(font.leading()));
            *glyph_count += line
                .glyph_runs()
                .iter()
                .map(|run| usize::try_from(run.glyph_count()).unwrap_or(0))
                .sum::<usize>();
            run_fallbacks(&line, primary_font, fallbacks);
            start += count;
        }
        Ok(())
    }

    pub fn layout(node: &Node) -> Result<TextLayout, String> {
        let content = authored_text(node, "content").unwrap_or("");
        let size = authored_number(node, "size").unwrap_or(16.0).max(0.01);
        let max_width = authored_number(node, "max_width").filter(|v| *v > 0.0);
        let requested_font = authored_text(node, "font")
            .unwrap_or("system-ui")
            .to_string();
        let letter_spacing = authored_number(node, "letter_spacing").unwrap_or(0.0);
        let (ligatures, ligature_value) = ligature_mode(node)?;
        let (font, font_variations) = make_font(node, size)?;
        let resolved_font = font.display_name();
        let resolved_family = font.family_name();
        let postscript_font = font.postscript_name();
        let requested_normalized = normalize_font_name(&requested_font);
        let font_substituted = requested_font != "system-ui"
            && requested_font != "system"
            && ![
                normalize_font_name(&resolved_family),
                normalize_font_name(&resolved_font),
                normalize_font_name(&postscript_font),
            ]
            .iter()
            .any(|resolved| {
                !requested_normalized.is_empty()
                    && (resolved.contains(&requested_normalized)
                        || requested_normalized.contains(resolved))
            });

        let mut lines = Vec::new();
        let mut line_widths = Vec::new();
        let mut ascent = 0.0f64;
        let mut descent = 0.0f64;
        let mut leading = 0.0f64;
        let mut glyph_count = 0usize;
        let mut fallback_fonts = BTreeSet::new();
        for paragraph in content.split('\n') {
            layout_paragraph(
                paragraph,
                &font,
                &postscript_font,
                max_width,
                letter_spacing,
                ligature_value,
                &mut lines,
                &mut line_widths,
                &mut ascent,
                &mut descent,
                &mut leading,
                &mut glyph_count,
                &mut fallback_fonts,
            )?;
        }
        if lines.is_empty() {
            lines.push(String::new());
            line_widths.push(0.0);
        }
        if ascent == 0.0 && descent == 0.0 {
            ascent = font.ascent();
            descent = font.descent();
            leading = font.leading();
        }
        let natural_line_height = (ascent + descent + leading).max(0.01);
        let line_height = authored_number(node, "line_height")
            .filter(|v| *v > 0.0)
            .unwrap_or(natural_line_height);
        let width = line_widths.iter().copied().fold(0.0, f64::max);
        let height = ascent + descent + line_height * (lines.len().saturating_sub(1) as f64);
        let overflow = max_width.is_some_and(|limit| width > limit + 0.001);
        Ok(TextLayout {
            lines,
            line_widths,
            width,
            height,
            line_height,
            ascent,
            descent,
            leading,
            glyph_count,
            engine: "coretext".into(),
            requested_font,
            resolved_font,
            postscript_font,
            fallback_fonts: fallback_fonts.into_iter().collect(),
            font_substituted,
            letter_spacing,
            ligatures: ligatures.to_string(),
            font_variations,
            overflow,
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::*;

    fn em_factor(ch: char) -> f64 {
        if ch.is_whitespace() {
            0.33
        } else if ch.is_ascii() {
            0.56
        } else {
            1.0
        }
    }

    fn estimate(text: &str, size: f64, letter_spacing: f64) -> f64 {
        let base = text.chars().map(em_factor).sum::<f64>() * size;
        base + letter_spacing * text.chars().count().saturating_sub(1) as f64
    }

    pub fn layout(node: &Node) -> Result<TextLayout, String> {
        let content = authored_text(node, "content").unwrap_or("");
        let size = authored_number(node, "size").unwrap_or(16.0).max(0.0);
        let lines: Vec<String> = content.split('\n').map(str::to_string).collect();
        let letter_spacing = authored_number(node, "letter_spacing").unwrap_or(0.0);
        let line_widths: Vec<f64> = lines
            .iter()
            .map(|line| estimate(line, size, letter_spacing))
            .collect();
        let width = line_widths.iter().copied().fold(0.0, f64::max);
        let line_height = authored_number(node, "line_height").unwrap_or(size * 1.2);
        let ascent = size * 0.8;
        let descent = size * 0.2;
        let requested_font = authored_text(node, "font")
            .unwrap_or("system-ui")
            .to_string();
        let font_variations = node
            .authored
            .props
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("font_axis_")
                    .and_then(|tag| value.number().map(|number| (tag.to_string(), number)))
            })
            .collect();
        Ok(TextLayout {
            height: ascent + descent + line_height * (lines.len().saturating_sub(1) as f64),
            lines,
            line_widths,
            width,
            line_height,
            ascent,
            descent,
            leading: 0.0,
            glyph_count: content.chars().count(),
            engine: "heuristic-non-macos-fallback".into(),
            requested_font: requested_font.clone(),
            resolved_font: requested_font.clone(),
            postscript_font: requested_font,
            fallback_fonts: Vec::new(),
            font_substituted: false,
            letter_spacing,
            ligatures: authored_text(node, "ligatures")
                .unwrap_or("standard")
                .to_string(),
            font_variations,
            overflow: false,
        })
    }
}

pub fn layout_text_node(node: &Node) -> TextLayout {
    debug_assert!(node.kind == NodeKind::Text);
    platform::layout(node).unwrap_or_else(|error| TextLayout {
        lines: vec![authored_text(node, "content").unwrap_or("").to_string()],
        line_widths: vec![0.0],
        width: 0.0,
        height: authored_number(node, "size").unwrap_or(16.0),
        line_height: authored_number(node, "line_height").unwrap_or(16.0),
        ascent: authored_number(node, "size").unwrap_or(16.0) * 0.8,
        descent: authored_number(node, "size").unwrap_or(16.0) * 0.2,
        leading: 0.0,
        glyph_count: 0,
        engine: format!("coretext-error:{error}"),
        requested_font: authored_text(node, "font")
            .unwrap_or("system-ui")
            .to_string(),
        resolved_font: String::new(),
        postscript_font: String::new(),
        fallback_fonts: Vec::new(),
        font_substituted: false,
        letter_spacing: authored_number(node, "letter_spacing").unwrap_or(0.0),
        ligatures: authored_text(node, "ligatures")
            .unwrap_or("standard")
            .to_string(),
        font_variations: node
            .authored
            .props
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("font_axis_")
                    .and_then(|tag| value.number().map(|number| (tag.to_string(), number)))
            })
            .collect(),
        overflow: true,
    })
}

pub fn apply_text_metrics(node: &mut Node) {
    if node.kind != NodeKind::Text {
        return;
    }
    let layout = layout_text_node(node);
    let variation_summary = layout
        .font_variations
        .iter()
        .map(|(tag, value)| format!("{tag}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    node.derived.props.insert("width".into(), layout.width);
    node.derived.props.insert("height".into(), layout.height);
    node.derived
        .props
        .insert("line_height".into(), layout.line_height);
    node.derived
        .props
        .insert("line_count".into(), layout.lines.len() as f64);
    node.derived
        .props
        .insert("glyph_count".into(), layout.glyph_count as f64);
    node.derived.props.insert("ascent".into(), layout.ascent);
    node.derived.props.insert("descent".into(), layout.descent);
    node.derived.props.insert("leading".into(), layout.leading);
    if let Some(y) = authored_number(node, "y") {
        node.derived.props.insert("baseline".into(), y);
    }
    node.derived
        .diagnostics
        .insert("text.engine".into(), layout.engine);
    node.derived
        .diagnostics
        .insert("text.font.requested".into(), layout.requested_font);
    node.derived
        .diagnostics
        .insert("text.font.resolved".into(), layout.resolved_font);
    node.derived
        .diagnostics
        .insert("text.font.postscript".into(), layout.postscript_font);
    node.derived.diagnostics.insert(
        "text.font.fallbacks".into(),
        layout.fallback_fonts.join(","),
    );
    node.derived.diagnostics.insert(
        "text.font.substituted".into(),
        layout.font_substituted.to_string(),
    );
    node.derived.diagnostics.insert(
        "text.letter_spacing".into(),
        layout.letter_spacing.to_string(),
    );
    node.derived
        .diagnostics
        .insert("text.ligatures".into(), layout.ligatures);
    node.derived
        .diagnostics
        .insert("text.font.variations".into(), variation_summary);
    node.derived.diagnostics.insert(
        "text.fit".into(),
        if layout.overflow { "overflow" } else { "fit" }.into(),
    );
}

pub fn text_left(node: &Node) -> f64 {
    let x = node
        .derived
        .props
        .get("x")
        .copied()
        .or_else(|| authored_number(node, "x"))
        .unwrap_or(0.0);
    let w = node
        .derived
        .props
        .get("width")
        .copied()
        .unwrap_or_else(|| layout_text_node(node).width);
    match authored_text(node, "align").unwrap_or("start") {
        "center" => x - w / 2.0,
        "right" => x - w,
        _ => x,
    }
}
