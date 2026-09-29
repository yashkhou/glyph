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

fn inspect_text(item: &DisplayItem, svg: &mut BackendConformance) {
    let DisplayPrimitive::Text { .. } = &item.primitive else {
        return;
    };
    issue(
        svg,
        item,
        "text_rasterization",
        ConformanceStatus::Degraded,
        "CoreText line breaks and typography controls are encoded, but SVG rasterization may reshape glyphs differently",
    );
}

pub fn renderer_conformance_report(scene: &Scene) -> RendererConformanceReport {
    let display = build_display_list(scene);
    let mut svg = BackendConformance {
        backend: "svg".into(),
        overall: ConformanceStatus::Exact,
        pixel_fidelity_guaranteed: false,
        issues: Vec::new(),
    };
    for item in &display.items {
        inspect_text(item, &mut svg);
    }
    RendererConformanceReport {
        display_list_schema: display.schema_version,
        svg,
    }
}
