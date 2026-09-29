use crate::{
    codegen::{SemanticCodegenTarget, assess_semantic_codegen},
    conformance::{ConformanceStatus, renderer_conformance_report},
    model::Scene,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetClass {
    VisualExport,
    SemanticCodegen,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetAvailability {
    Implemented,
    StrategyOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetAssessment {
    pub target: String,
    pub class: TargetClass,
    pub availability: TargetAvailability,
    pub status: ConformanceStatus,
    pub lowering: String,
    pub semantic_identity_preserved: bool,
    pub editable_output: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetStrategyReport {
    pub display_list_schema: u32,
    pub targets: Vec<TargetAssessment>,
}

fn visual_target(
    target: &str,
    lowering: &str,
    status: ConformanceStatus,
    semantic_identity_preserved: bool,
    editable_output: bool,
    notes: Vec<String>,
) -> TargetAssessment {
    TargetAssessment {
        target: target.into(),
        class: TargetClass::VisualExport,
        availability: TargetAvailability::Implemented,
        status,
        lowering: lowering.into(),
        semantic_identity_preserved,
        editable_output,
        notes,
    }
}

fn codegen_target(
    scene: &Scene,
    target: SemanticCodegenTarget,
    lowering: &str,
) -> TargetAssessment {
    let assessment = assess_semantic_codegen(scene, target);
    let mut notes = assessment.warnings;
    notes.extend(
        assessment
            .issues
            .into_iter()
            .map(|issue| format!("{}.{}: {}", issue.item_id, issue.feature, issue.detail)),
    );
    TargetAssessment {
        target: target.as_str().into(),
        class: TargetClass::SemanticCodegen,
        availability: TargetAvailability::Implemented,
        status: assessment.status,
        lowering: lowering.into(),
        semantic_identity_preserved: true,
        editable_output: true,
        notes,
    }
}

pub fn target_strategy_report(scene: &Scene) -> TargetStrategyReport {
    let conformance = renderer_conformance_report(scene);
    let svg_issues = conformance
        .svg
        .issues
        .iter()
        .map(|issue| format!("{}: {}", issue.item_id, issue.detail))
        .collect();
    let svg = visual_target(
        "svg",
        "display_list -> native SVG",
        conformance.svg.overall,
        true,
        true,
        svg_issues,
    );
    let swiftui = codegen_target(
        scene,
        SemanticCodegenTarget::SwiftUi,
        "display_list bounded subset -> native SwiftUI view code",
    );
    let web_canvas = codegen_target(
        scene,
        SemanticCodegenTarget::WebCanvas,
        "display_list bounded subset -> editable browser Canvas module",
    );
    TargetStrategyReport {
        display_list_schema: conformance.display_list_schema,
        targets: vec![svg, swiftui, web_canvas],
    }
}
