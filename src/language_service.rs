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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolReference {
    pub name: String,
    pub range: TextRange,
    pub declaration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceTextEdit {
    pub range: TextRange,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenamePlan {
    pub symbol: String,
    pub edits: Vec<SourceTextEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanguageCodeAction {
    pub title: String,
    pub kind: String,
    pub edits: Vec<SourceTextEdit>,
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
pub(crate) fn byte_offset_at_position(source: &str, position: TextPosition) -> usize {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct LexToken {
    text: String,
    start: usize,
    end: usize,
    local_scope: bool,
}

fn position_at_byte_offset(source: &str, offset: usize) -> TextPosition {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let character = source[line_start..offset]
        .chars()
        .map(|ch| ch.len_utf16() as u32)
        .sum();
    TextPosition { line, character }
}

fn range_for_byte_span(source: &str, start: usize, end: usize) -> TextRange {
    TextRange {
        start: position_at_byte_offset(source, start),
        end: position_at_byte_offset(source, end),
    }
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/')
}

fn lex_tokens(source: &str) -> Vec<LexToken> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                    continue;
                }
                if bytes[index] == b'"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if is_ident_byte(bytes[index]) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_ident_byte(bytes[index]) {
                index += 1;
            }
            tokens.push(LexToken {
                text: source[start..index].into(),
                start,
                end: index,
                local_scope: false,
            });
            continue;
        }
        if matches!(bytes[index], b'{' | b'}' | b'.' | b';') {
            let start = index;
            index += 1;
            tokens.push(LexToken {
                text: source[start..index].into(),
                start,
                end: index,
                local_scope: false,
            });
            continue;
        }
        index += 1;
    }

    let mut local_stack = Vec::<bool>::new();
    for i in 0..tokens.len() {
        let inherited_local = local_stack.iter().any(|value| *value);
        tokens[i].local_scope = inherited_local;
        if tokens[i].text == "{" {
            let mut opener = None;
            let mut cursor = i;
            while cursor > 0 {
                cursor -= 1;
                if matches!(tokens[cursor].text.as_str(), "{" | "}" | ";") {
                    break;
                }
                if matches!(tokens[cursor].text.as_str(), "component" | "instance") {
                    opener = Some(tokens[cursor].text.as_str());
                    break;
                }
            }
            local_stack.push(inherited_local || opener.is_some());
        } else if tokens[i].text == "}" {
            local_stack.pop();
        }
    }
    tokens
}

fn is_node_kind(value: &str) -> bool {
    matches!(
        value,
        "rect" | "ellipse" | "path" | "text" | "image" | "group"
    )
}

fn top_level_node_declarations(tokens: &[LexToken]) -> Vec<usize> {
    (1..tokens.len())
        .filter(|index| {
            let token = &tokens[*index];
            !token.local_scope && is_node_kind(&tokens[*index - 1].text) && token.text != "{"
        })
        .collect()
}

fn is_supported_node_reference(tokens: &[LexToken], index: usize) -> bool {
    if tokens[index].local_scope {
        return false;
    }
    let previous = index
        .checked_sub(1)
        .and_then(|i| tokens.get(i))
        .map(|t| t.text.as_str());
    let next = tokens.get(index + 1).map(|t| t.text.as_str());
    next == Some(".") || matches!(previous, Some("member" | "repeat"))
}

fn symbol_reference_indexes(tokens: &[LexToken], symbol: &str) -> Vec<(usize, bool)> {
    let declarations = top_level_node_declarations(tokens);
    let declaration_set = declarations.iter().copied().collect::<BTreeSet<_>>();
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            if token.text != symbol {
                return None;
            }
            if declaration_set.contains(&index) {
                return Some((index, true));
            }
            is_supported_node_reference(tokens, index).then_some((index, false))
        })
        .collect()
}

pub(crate) fn node_symbol_at_position(
    source: &str,
    position: TextPosition,
) -> Option<(String, TextRange)> {
    let tokens = lex_tokens(source);
    let offset = byte_offset_at_position(source, position);
    let declaration_names = top_level_node_declarations(&tokens)
        .into_iter()
        .map(|index| tokens[index].text.clone())
        .collect::<BTreeSet<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if offset < token.start || offset > token.end {
            continue;
        }
        if !declaration_names.contains(&token.text) {
            continue;
        }
        let is_declaration =
            index > 0 && !token.local_scope && is_node_kind(&tokens[index - 1].text);
        if is_declaration || is_supported_node_reference(&tokens, index) {
            return Some((
                token.text.clone(),
                range_for_byte_span(source, token.start, token.end),
            ));
        }
    }
    None
}

pub fn references_at(
    source: &str,
    position: TextPosition,
    include_declaration: bool,
) -> Vec<SymbolReference> {
    let Some((symbol, _)) = node_symbol_at_position(source, position) else {
        return vec![];
    };
    let tokens = lex_tokens(source);
    symbol_reference_indexes(&tokens, &symbol)
        .into_iter()
        .filter(|(_, declaration)| include_declaration || !*declaration)
        .map(|(index, declaration)| SymbolReference {
            name: symbol.clone(),
            range: range_for_byte_span(source, tokens[index].start, tokens[index].end),
            declaration,
        })
        .collect()
}

pub fn definition_at(source: &str, position: TextPosition) -> Option<SymbolReference> {
    references_at(source, position, true)
        .into_iter()
        .find(|reference| reference.declaration)
}

fn valid_rename_ident(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(is_ident_byte)
        && !matches!(
            value,
            "scene"
                | "canvas"
                | "component"
                | "instance"
                | "state"
                | "style"
                | "paint"
                | "rect"
                | "ellipse"
                | "path"
                | "text"
                | "image"
                | "group"
                | "constrain"
                | "member"
                | "repeat"
                | "animate"
                | "assert"
                | "import"
        )
}

pub fn rename_node_at(
    source: &str,
    position: TextPosition,
    new_name: &str,
) -> Result<RenamePlan, String> {
    if !valid_rename_ident(new_name) {
        return Err(format!("{new_name:?} is not a safe Glyph node identifier"));
    }
    let Some((symbol, _)) = node_symbol_at_position(source, position) else {
        return Err("position is not a renameable top-level node symbol".into());
    };
    if symbol == new_name {
        return Ok(RenamePlan {
            symbol,
            edits: vec![],
        });
    }
    let tokens = lex_tokens(source);
    let declarations = top_level_node_declarations(&tokens);
    if declarations
        .iter()
        .filter(|index| tokens[**index].text == symbol)
        .count()
        != 1
    {
        return Err(format!(
            "node name {symbol:?} is ambiguous in this document"
        ));
    }
    if declarations
        .iter()
        .any(|index| tokens[*index].text == new_name)
    {
        return Err(format!("top-level node {new_name:?} already exists"));
    }
    let edits = symbol_reference_indexes(&tokens, &symbol)
        .into_iter()
        .map(|(index, _)| SourceTextEdit {
            range: range_for_byte_span(source, tokens[index].start, tokens[index].end),
            new_text: new_name.into(),
        })
        .collect::<Vec<_>>();
    Ok(RenamePlan { symbol, edits })
}

pub fn apply_source_text_edits(source: &str, edits: &[SourceTextEdit]) -> Result<String, String> {
    let mut spans = edits
        .iter()
        .map(|edit| {
            let start = byte_offset_at_position(source, edit.range.start);
            let end = byte_offset_at_position(source, edit.range.end);
            if start > end {
                return Err("text edit range is reversed".to_string());
            }
            Ok((start, end, edit.new_text.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    spans.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut edited = source.to_string();
    let mut previous_start = source.len() + 1;
    for (start, end, replacement) in spans {
        if end > previous_start {
            return Err("text edits overlap".into());
        }
        if !edited.is_char_boundary(start) || !edited.is_char_boundary(end) {
            return Err("text edit does not align to UTF-8 boundaries".into());
        }
        edited.replace_range(start..end, &replacement);
        previous_start = start;
    }
    Ok(edited)
}

fn any_node_declaration_on_line(source: &str, line: u32) -> Option<(String, usize)> {
    let tokens = lex_tokens(source);
    for index in 1..tokens.len() {
        if !is_node_kind(&tokens[index - 1].text) {
            continue;
        }
        if position_at_byte_offset(source, tokens[index].start).line != line {
            continue;
        }
        let mut cursor = index + 1;
        while cursor < tokens.len() && tokens[cursor].text != "{" {
            if matches!(tokens[cursor].text.as_str(), ";" | "}") {
                break;
            }
            cursor += 1;
        }
        if cursor < tokens.len() && tokens[cursor].text == "{" {
            return Some((tokens[index].text.clone(), tokens[cursor].end));
        }
    }
    None
}

fn humanized_accessible_name(name: &str) -> String {
    let words = name
        .split(['_', '-', '/'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    if words.is_empty() {
        "Item".into()
    } else {
        words.join(" ")
    }
}

pub fn code_actions(source: &str, diagnostics: &[LanguageDiagnostic]) -> Vec<LanguageCodeAction> {
    let mut actions = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.code != "glyph.accessibility.missing_accessible_name" {
            continue;
        }
        let Some((name, insert_offset)) =
            any_node_declaration_on_line(source, diagnostic.range.start.line)
        else {
            continue;
        };
        let label = humanized_accessible_name(&name).replace('"', "\\\"");
        let range = range_for_byte_span(source, insert_offset, insert_offset);
        actions.push(LanguageCodeAction {
            title: format!("Add accessible name to {name}"),
            kind: "quickfix".into(),
            edits: vec![SourceTextEdit {
                range,
                new_text: format!(" a11y_label \"{label}\";"),
            }],
        });
    }
    actions
}
