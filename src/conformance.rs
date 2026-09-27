use crate::display_list::{DisplayItem, DisplayPrimitive, build_display_list};
use crate::model::Scene;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceStatus {
    Exact,
    Degraded,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConformanceIssue {
    pub item_id: String,
    pub feature: String,
    pub status: ConformanceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendConformance {
    pub backend: String,
    pub overall: ConformanceStatus,
    pub pixel_fidelity_guaranteed: bool,
    pub issues: Vec<ConformanceIssue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RendererConformanceReport {
    pub display_list_schema: u32,
    pub svg: BackendConformance,
    pub commander_draw: BackendConformance,
}

fn issue(
    backend: &mut BackendConformance,
    item: &DisplayItem,
    feature: &str,
    status: ConformanceStatus,
    detail: impl Into<String>,
) {
    backend.overall = backend.overall.max(status);
    backend.issues.push(ConformanceIssue {
        item_id: item.id.clone(),
        feature: feature.into(),
        status,
        detail: detail.into(),
    });
}

fn has_transform(item: &DisplayItem) -> bool {
    let t = &item.transform;
    t.translate_x != 0.0
        || t.translate_y != 0.0
        || t.rotate_deg != 0.0
        || t.scale_x != 1.0
        || t.scale_y != 1.0
}
fn inspect_text(item: &DisplayItem, svg: &mut BackendConformance, draw: &mut BackendConformance) {
    let DisplayPrimitive::Text {
        font_substituted,
        fallback_fonts,
        ligatures,
        font_variations,
        ..
    } = &item.primitive
    else {
        return;
    };

    issue(
        svg,
        item,
        "text_rasterization",
        ConformanceStatus::Degraded,
        "CoreText line breaks and typography controls are encoded, but SVG rasterization may reshape glyphs differently",
    );
    issue(
        draw,
        item,
        "text_rasterization",
        ConformanceStatus::Degraded,
        "CoreText metrics are preserved as metadata, but CommanderDraw does not guarantee identical glyph shaping",
    );
    if *font_substituted || !fallback_fonts.is_empty() {
        issue(
            draw,
            item,
            "font_resolution",
            ConformanceStatus::Degraded,
            "resolved/fallback font metadata is preserved, but the backend font cascade may differ",
        );
    }
    if ligatures != "standard" {
        issue(
            draw,
            item,
            "ligatures",
            ConformanceStatus::Degraded,
            "ligature policy is preserved as metadata but is not guaranteed by CommanderDraw v1",
        );
    }
    if !font_variations.is_empty() {
        issue(
            draw,
            item,
            "variable_fonts",
            ConformanceStatus::Degraded,
            "variable-font axes are preserved as metadata but are not guaranteed by CommanderDraw v1",
        );
    }
}

pub fn renderer_conformance_report(scene: &Scene) -> RendererConformanceReport {
    let display = build_display_list(scene);
    let mut svg = BackendConformance {
        backend: "svg".into(),
        overall: ConformanceStatus::Exact,
        pixel_fidelity_guaranteed: false,
        issues: Vec::new(),
    };
    let mut commander_draw = BackendConformance {
        backend: "commander_draw".into(),
        overall: ConformanceStatus::Exact,
        pixel_fidelity_guaranteed: false,
        issues: Vec::new(),
    };

    for item in &display.items {
        inspect_text(item, &mut svg, &mut commander_draw);
        if matches!(item.primitive, DisplayPrimitive::Group) {
            issue(
                &mut commander_draw,
                item,
                "group_identity",
                ConformanceStatus::Degraded,
                "CommanderDraw v1 omits non-rendering Glyph group nodes, so group semantic identity is not exported",
            );
        }
        if item.definition_only {
            issue(
                &mut commander_draw,
                item,
                "definition_identity",
                ConformanceStatus::Degraded,
                "clip/mask definition geometry is lowered into consumers rather than exported as a stable standalone node",
            );
        }
        if item.mask.is_some() {
            issue(
                &mut commander_draw,
                item,
                "mask",
                ConformanceStatus::Unsupported,
                "CommanderDraw v1 cannot faithfully represent Glyph masks",
            );
        }
        if let Some(clip_id) = &item.clip
            && let Some(target) = display
                .items
                .iter()
                .find(|candidate| candidate.id == *clip_id)
            && has_transform(target)
        {
            issue(
                &mut commander_draw,
                item,
                "transformed_clip",
                ConformanceStatus::Unsupported,
                format!(
                    "clip target {clip_id} has a transform that CommanderDraw clipPath lowering cannot preserve"
                ),
            );
        }
    }

    RendererConformanceReport {
        display_list_schema: display.schema_version,
        svg,
        commander_draw,
    }
}
