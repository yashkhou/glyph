use crate::{
    accessibility::accessibility_report,
    model::Scene,
    parser::{parse_import_specs, parse_scene, parse_scene_with_environment},
    project::load_project_scene_with_source,
    solver::solve_scene,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageDiagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub range: TextRange,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String,
    pub detail: String,
    pub insert_text: String,
}

fn one_line_range(line: u32, character: u32) -> TextRange {
    TextRange {
        start: TextPosition { line, character },
        end: TextPosition {
            line,
            character: character.saturating_add(1),
        },
    }
}

fn parse_error_position(message: &str) -> TextPosition {
    let Some(marker) = message.find("--> ") else {
        return TextPosition::default();
    };
    let rest = &message[marker + 4..];
    let location = rest.lines().next().unwrap_or_default().trim();
    let Some((line, column)) = location.split_once(':') else {
        return TextPosition::default();
    };
    TextPosition {
        line: line.parse::<u32>().unwrap_or(1).saturating_sub(1),
        character: column.parse::<u32>().unwrap_or(1).saturating_sub(1),
    }
}
pub fn diagnose_source(source: &str, source_name: &str) -> Vec<LanguageDiagnostic> {
    let imports = parse_import_specs(source).unwrap_or_default();
    let parsed = if imports.is_empty() {
        parse_scene(source, source_name)
    } else {
        parse_scene_with_environment(
            source,
            source_name,
            BTreeMap::new(),
            BTreeMap::new(),
            Vec::new(),
            imports,
        )
    };
    let mut scene = match parsed {
        Ok(scene) => scene,
        Err(error) => {
            let message = format!("{error:#}");
            let position = parse_error_position(&message);
            return vec![LanguageDiagnostic {
                severity: DiagnosticSeverity::Error,
                code: "glyph.parse".into(),
                message,
                range: one_line_range(position.line, position.character),
            }];
        }
    };
    diagnose_scene(&mut scene)
}

pub fn diagnose_project_source(path: &Path, source: &str) -> Vec<LanguageDiagnostic> {
    match load_project_scene_with_source(path, source) {
        Ok(mut scene) => diagnose_scene(&mut scene),
        Err(error) => {
            let message = format!("{error:#}");
            let position = parse_error_position(&message);
            vec![LanguageDiagnostic {
                severity: DiagnosticSeverity::Error,
                code: "glyph.project".into(),
                message,
                range: one_line_range(position.line, position.character),
            }]
        }
    }
}

fn diagnose_scene(scene: &mut Scene) -> Vec<LanguageDiagnostic> {
    let solve = solve_scene(scene);
    let mut diagnostics = Vec::new();
    for id in &solve.required_conflicts {
        diagnostics.push(LanguageDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: "glyph.required_constraint_conflict".into(),
            message: format!("required constraint conflict: {id}"),
            range: TextRange::default(),
        });
    }
    for id in &solve.unresolved {
        diagnostics.push(LanguageDiagnostic {
            severity: DiagnosticSeverity::Error,
            code: "glyph.unresolved_constraint".into(),
            message: format!("unresolved constraint: {id}"),
            range: TextRange::default(),
        });
    }
    for issue in accessibility_report(scene).issues {
        let line = scene
            .nodes
            .iter()
            .find(|node| node.id == issue.node_id)
            .map(|node| node.provenance.line.saturating_sub(1) as u32)
            .unwrap_or(0);
        diagnostics.push(LanguageDiagnostic {
            severity: match issue.severity {
                crate::accessibility::AccessibilitySeverity::Error => DiagnosticSeverity::Error,
                crate::accessibility::AccessibilitySeverity::Warning => DiagnosticSeverity::Warning,
            },
            code: format!("glyph.accessibility.{}", issue.code),
            message: issue.message,
            range: one_line_range(line, 0),
        });
    }
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic.range.start.line,
            diagnostic.range.start.character,
            diagnostic.code.clone(),
        )
    });
    diagnostics
}
fn byte_offset_at_position(source: &str, position: TextPosition) -> usize {
    let mut offset = 0usize;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index as u32 == position.line {
            let content = line.strip_suffix('\n').unwrap_or(line);
            let mut utf16 = 0u32;
            for (byte, ch) in content.char_indices() {
                if utf16 >= position.character {
                    return offset + byte;
                }
                utf16 += ch.len_utf16() as u32;
            }
            return offset + content.len();
        }
        offset += line.len();
    }
    source.len()
}

fn declaration_kind_before_brace(fragment: &str) -> Option<&'static str> {
    let trimmed = fragment.trim();
    for kind in ["rect", "ellipse", "path", "text", "image", "group"] {
        if trimmed.starts_with(kind) && trimmed[kind.len()..].starts_with(char::is_whitespace) {
            return Some(kind);
        }
    }
    None
}
fn enclosing_node_kind(source: &str, offset: usize) -> Option<&'static str> {
    let mut stack: Vec<Option<&'static str>> = Vec::new();
    let mut quote = false;
    let mut escaped = false;
    let mut statement_start = 0usize;
    for (index, ch) in source[..offset.min(source.len())].char_indices() {
        if quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quote = false;
            }
            continue;
        }
        if ch == '"' {
            quote = true;
            continue;
        }
        match ch {
            '{' => {
                let kind = declaration_kind_before_brace(&source[statement_start..index]);
                stack.push(kind);
                statement_start = index + 1;
            }
            '}' => {
                stack.pop();
                statement_start = index + 1;
            }
            ';' | '\n' => statement_start = index + 1,
            _ => {}
        }
    }
    stack.into_iter().rev().flatten().next()
}

fn declared_nodes(source: &str) -> Vec<String> {
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        for kind in ["rect", "ellipse", "path", "text", "image", "group"] {
            if let Some(rest) = trimmed
                .strip_prefix(kind)
                .and_then(|rest| rest.strip_prefix(' '))
            {
                if let Some(name) = rest.split_whitespace().next() {
                    names.insert(name.to_string());
                }
            }
        }
    }
    names.into_iter().collect()
}
fn completion(label: &str, kind: &str, detail: &str, insert_text: &str) -> CompletionItem {
    CompletionItem {
        label: label.into(),
        kind: kind.into(),
        detail: detail.into(),
        insert_text: insert_text.into(),
    }
}

fn common_node_completions() -> Vec<CompletionItem> {
    [
        ("role", "role \"semantic-role\";"),
        ("fill", "fill \"#000000\";"),
        ("stroke", "stroke \"#000000\";"),
        ("opacity", "opacity 1;"),
        ("a11y_label", "a11y_label \"Label\";"),
        ("a11y_role", "a11y_role \"button\";"),
        ("reading_order", "reading_order 0;"),
    ]
    .into_iter()
    .map(|(label, insert)| completion(label, "property", "Glyph node property", insert))
    .collect()
}

fn node_specific(kind: &str) -> Vec<CompletionItem> {
    let properties: &[(&str, &str)] = match kind {
        "rect" | "image" => &[
            ("x", "x 0px;"),
            ("y", "y 0px;"),
            ("width", "width 100px;"),
            ("height", "height 100px;"),
        ],
        "ellipse" => &[
            ("cx", "cx 0px;"),
            ("cy", "cy 0px;"),
            ("rx", "rx 50px;"),
            ("ry", "ry 50px;"),
        ],
        "text" => &[
            ("content", "content \"Text\";"),
            ("x", "x 0px;"),
            ("y", "y 0px;"),
            ("size", "size 16px;"),
            ("max_width", "max_width 240px;"),
        ],
        "path" => &[
            ("d", "d \"M0 0 L100 0 L100 100 Z\";"),
            ("translate_x", "translate_x 0px;"),
            ("translate_y", "translate_y 0px;"),
            ("scale_x", "scale_x 1;"),
            ("scale_y", "scale_y 1;"),
        ],
        _ => &[],
    };
    properties
        .iter()
        .map(|(label, insert)| completion(label, "property", "Glyph node property", insert))
        .collect()
}
pub fn complete_source(source: &str, position: TextPosition) -> Vec<CompletionItem> {
    let offset = byte_offset_at_position(source, position);
    let line_start = source[..offset]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = source[line_start..offset].trim_start();
    let names = declared_nodes(source);

    if prefix.starts_with("member ") {
        return names
            .into_iter()
            .map(|name| {
                completion(
                    &name,
                    "reference",
                    "Structural group member",
                    &format!("{name};"),
                )
            })
            .collect();
    }
    if prefix.starts_with("constrain ") && !prefix.contains('.') {
        let mut values = vec![completion(
            "canvas",
            "reference",
            "Canvas constraint target",
            "canvas.",
        )];
        values.extend(names.into_iter().map(|name| {
            completion(
                &name,
                "reference",
                "Glyph node constraint target",
                &format!("{name}."),
            )
        }));
        return values;
    }
    if let Some(kind) = enclosing_node_kind(source, offset) {
        let mut values = common_node_completions();
        values.extend(node_specific(kind));
        if kind == "group" {
            values.push(completion(
                "member",
                "keyword",
                "Add structural group member",
                "member ",
            ));
        }
        values.sort_by(|a, b| a.label.cmp(&b.label));
        values.dedup_by(|a, b| a.label == b.label);
        return values;
    }
    [
        ("canvas", "canvas 1024px 1024px;"),
        ("rect", "rect name @id(\"name\") {\n  x 0px; y 0px; width 100px; height 100px;\n}"),
        ("ellipse", "ellipse name @id(\"name\") {\n  cx 50px; cy 50px; rx 50px; ry 50px;\n}"),
        ("path", "path name @id(\"name\") {\n  d \"M0 0 L100 0 L100 100 Z\";\n}"),
        ("text", "text name @id(\"name\") {\n  content \"Text\"; x 0px; y 20px; size 16px;\n}"),
        ("image", "image name @id(\"name\") {\n  x 0px; y 0px; width 100px; height 100px; src \"asset.png\";\n}"),
        ("group", "group name @id(\"name\") {\n  member child;\n}"),
        ("constrain", "constrain node.left == canvas.left @required;"),
        ("style", "style name {\n  fill \"#000000\";\n}"),
        ("component", "component name {\n}"),
    ]
    .into_iter()
    .map(|(label, insert)| completion(label, "keyword", "Glyph scene item", insert))
    .collect()
}
