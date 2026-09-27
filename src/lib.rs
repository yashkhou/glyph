pub mod accessibility;
pub mod bridge;
pub mod codegen;
pub mod conformance;
pub mod diff;
pub mod display_list;
pub mod language_service;
mod linear_solver;
pub mod lsp;
pub mod manipulation;
pub mod migration;
pub mod model;
pub mod parser;
pub mod patch;
pub mod project;
pub mod query;
pub mod render;
pub mod runtime;
pub mod solver;
pub mod source_edit;
pub mod targets;
pub mod text_layout;
pub mod vision;

pub use accessibility::{
    AccessibilityIssue, AccessibilityReport, AccessibilitySeverity, accessibility_report,
};
pub use bridge::commander_draw_document;
pub use codegen::{
    SemanticCodegenAssessment, SemanticCodegenTarget, assess_semantic_codegen, render_swiftui,
    render_web_canvas,
};
pub use conformance::{
    BackendConformance, ConformanceIssue, ConformanceStatus, RendererConformanceReport,
    renderer_conformance_report,
};
pub use display_list::{
    DisplayAccessibility, DisplayCanvas, DisplayItem, DisplayList, DisplayPrimitive, DisplayStyle,
    DisplayTransform, ObjectRecord, build_display_list,
};
pub use language_service::{
    CompletionItem, DiagnosticSeverity, LanguageCodeAction, LanguageDiagnostic, LanguageHover,
    LanguageSemanticToken, ProjectRenamePlan, ProjectSourceTextEdit, ProjectSymbolKind,
    ProjectSymbolLocation, RenamePlan, SemanticTokenKind, SourceTextEdit, SymbolReference,
    TextPosition, TextRange, apply_source_text_edits, code_actions, complete_source, definition_at,
    diagnose_project_source, diagnose_source, hover_at, project_definition_at, project_hover_at,
    project_references_at, references_at, rename_node_at, rename_project_symbol_at,
    semantic_tokens,
};
pub use manipulation::{
    ComponentParameterEdit, ConstraintOffsetEdit, DirectHandleResizePlan, DirectManipulationPlan,
    DirectResizePlan, ManipulationAxis, ManipulationBlock, ManipulationEdit, ResizeEdit,
    ResizeHandle, apply_direct_handle_resize_source, apply_direct_manipulation_source,
    apply_direct_resize_source, plan_direct_manipulation, plan_direct_resize,
    plan_direct_resize_from_handle,
};
pub use migration::{MigrationReport, load_gsir_str, migrate_gsir_value};
pub use model::*;
pub use parser::{parse_scene, parse_value_literal};
pub use patch::{PatchOperation, TransactionResult, apply_transaction};
pub use project::{
    ProjectAsset, ProjectAssetRecord, ProjectBundleManifest, ProjectFileRecord, ProjectManifest,
    ProjectValidationReport, discover_project_root, load_project_scene,
    load_project_scene_with_source, project_bundle_manifest, validate_project,
    write_project_bundle, write_project_bundle_manifest,
};
pub use render::{render_svg, write_svg};
pub use runtime::{AssertionResult, evaluate_assertions, sample_motion, scene_in_state};
pub use solver::{SolveReport, solve_scene};
pub use source_edit::{
    ComponentBindingSummary, ComponentCatalogEntry, DirectSourceNode, NodePlacement,
    add_source_constraint, add_source_group_member, component_catalog, create_source_group,
    create_source_instance, direct_source_nodes, extract_source_component, move_source_node,
    remove_source_constraint, remove_source_group, remove_source_group_member,
    remove_source_instance, reparent_source_node, set_source_constraint_offset,
    set_source_constraint_relation, set_source_constraint_strength, set_source_instance_parameter,
    set_source_instance_slot, set_source_property,
};
pub use targets::{
    TargetAssessment, TargetAvailability, TargetClass, TargetStrategyReport, target_strategy_report,
};
pub use text_layout::{TextLayout, layout_text_node};
