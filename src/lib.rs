pub mod bridge;
pub mod conformance;
pub mod diff;
pub mod display_list;
mod linear_solver;
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

pub use bridge::commander_draw_document;
pub use conformance::{
    BackendConformance, ConformanceIssue, ConformanceStatus, RendererConformanceReport,
    renderer_conformance_report,
};
pub use display_list::{
    DisplayCanvas, DisplayItem, DisplayList, DisplayPrimitive, DisplayStyle, DisplayTransform,
    ObjectRecord, build_display_list,
};
pub use manipulation::{
    ComponentParameterEdit, ConstraintOffsetEdit, DirectManipulationPlan, DirectResizePlan,
    ManipulationAxis, ManipulationBlock, ManipulationEdit, ResizeEdit,
    apply_direct_manipulation_source, apply_direct_resize_source, plan_direct_manipulation,
    plan_direct_resize,
};
pub use migration::{MigrationReport, load_gsir_str, migrate_gsir_value};
pub use model::*;
pub use parser::{parse_scene, parse_value_literal};
pub use patch::{PatchOperation, TransactionResult, apply_transaction};
pub use project::{discover_project_root, load_project_scene};
pub use render::{render_svg, write_svg};
pub use runtime::{AssertionResult, evaluate_assertions, sample_motion, scene_in_state};
pub use solver::{SolveReport, solve_scene};
pub use source_edit::{
    add_source_constraint, remove_source_constraint, set_source_constraint_offset,
    set_source_constraint_relation, set_source_constraint_strength, set_source_property,
};
pub use targets::{
    TargetAssessment, TargetAvailability, TargetClass, TargetStrategyReport, target_strategy_report,
};
pub use text_layout::{TextLayout, layout_text_node};
