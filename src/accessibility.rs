use crate::{display_list::build_display_list, model::Scene};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessibilitySeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibilityIssue {
    pub node_id: String,
    pub code: String,
    pub severity: AccessibilitySeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessibilityReport {
    pub scene_id: String,
    pub accessible_nodes: usize,
    pub interactive_nodes: usize,
    pub reading_order: Vec<String>,
    pub issues: Vec<AccessibilityIssue>,
}

fn issue(
    node_id: &str,
    code: &str,
    severity: AccessibilitySeverity,
    message: impl Into<String>,
) -> AccessibilityIssue {
    AccessibilityIssue {
        node_id: node_id.into(),
        code: code.into(),
        severity,
        message: message.into(),
    }
}

fn role_needs_name(role: &str) -> bool {
    matches!(
        role,
        "button" | "link" | "checkbox" | "radio" | "switch" | "img" | "image" | "textbox"
    )
}

pub fn accessibility_report(scene: &Scene) -> AccessibilityReport {
    let display = build_display_list(scene);
    let mut issues = Vec::new();
    let mut explicit_orders: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    let mut interactive_nodes = 0usize;
    let mut accessible_nodes = 0usize;
    for item in &display.items {
        let a11y = &item.accessibility;
        if a11y.hidden {
            if a11y.interactive {
                issues.push(issue(
                    &item.id,
                    "hidden_interactive",
                    AccessibilitySeverity::Error,
                    "an accessibility-hidden node cannot also be interactive",
                ));
            }
            continue;
        }
        if a11y.label.is_some() || a11y.labelled_by.is_some() || a11y.role.is_some() {
            accessible_nodes += 1;
        }
        if let Some(order) = a11y.reading_order {
            explicit_orders
                .entry(order)
                .or_default()
                .push(item.id.clone());
        }
        if let Some(target) = &a11y.labelled_by
            && !display.object_map.contains_key(target)
        {
            issues.push(issue(
                &item.id,
                "missing_label_target",
                AccessibilitySeverity::Error,
                format!("a11y_labelled_by references missing node {target}"),
            ));
        }
        let needs_name = a11y.interactive || a11y.role.as_deref().is_some_and(role_needs_name);
        if needs_name && a11y.label.is_none() && a11y.labelled_by.is_none() {
            issues.push(issue(
                &item.id,
                "missing_accessible_name",
                AccessibilitySeverity::Error,
                "interactive/named-role node needs a11y_label or a11y_labelled_by",
            ));
        }
        if a11y.interactive {
            interactive_nodes += 1;
            match item.bounds {
                Some([_, _, width, height]) if width >= 44.0 && height >= 44.0 => {}
                Some([_, _, width, height]) => issues.push(issue(
                    &item.id,
                    "small_interactive_target",
                    AccessibilitySeverity::Warning,
                    format!(
                        "interactive target is {width:.1}×{height:.1}; 44×44 is the default Glyph audit floor"
                    ),
                )),
                None => issues.push(issue(
                    &item.id,
                    "unmeasurable_interactive_target",
                    AccessibilitySeverity::Error,
                    "interactive target has no resolved bounds",
                )),
            }
        }
    }

    for (order, ids) in &explicit_orders {
        if ids.len() > 1 {
            for id in ids {
                issues.push(issue(
                    id,
                    "duplicate_reading_order",
                    AccessibilitySeverity::Error,
                    format!("reading_order {order} is also used by {}", ids.join(", ")),
                ));
            }
        }
    }

    let mut reading_order = explicit_orders
        .values()
        .flat_map(|ids| ids.iter().cloned())
        .collect::<Vec<_>>();
    reading_order.extend(
        display
            .items
            .iter()
            .filter(|item| !item.accessibility.hidden && item.accessibility.reading_order.is_none())
            .map(|item| item.id.clone()),
    );

    AccessibilityReport {
        scene_id: scene.id.clone(),
        accessible_nodes,
        interactive_nodes,
        reading_order,
        issues,
    }
}
