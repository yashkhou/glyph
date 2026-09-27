use crate::language_service::{
    DiagnosticSeverity, LanguageDiagnostic, SemanticTokenKind, TextPosition, TextRange,
    byte_offset_at_position, code_actions, complete_source, definition_at, diagnose_project_source,
    diagnose_source, node_symbol_at_position, project_definition_at, project_hover_at,
    project_references_at, references_at, rename_node_at, rename_project_symbol_at,
    semantic_tokens,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, BufReader, Read, Write},
    path::PathBuf,
};

fn read_message(reader: &mut BufReader<io::StdinLock<'_>>) -> Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse::<usize>()?);
        }
    }
    let length = content_length.ok_or_else(|| anyhow!("missing LSP Content-Length header"))?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(
        serde_json::from_slice(&body).context("parse LSP JSON-RPC message")?,
    ))
}
fn write_message(writer: &mut io::StdoutLock<'_>, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

fn percent_decode_uri_path(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let hi = (bytes[index + 1] as char).to_digit(16)? as u8;
            let lo = (bytes[index + 2] as char).to_digit(16)? as u8;
            output.push((hi << 4) | lo);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let raw_path = if rest.starts_with('/') {
        rest
    } else if let Some(path) = rest.strip_prefix("localhost") {
        path
    } else {
        return None;
    };
    percent_decode_uri_path(raw_path).map(PathBuf::from)
}

fn diagnostic_notification(uri: &str, source: &str) -> Value {
    let diagnostics = file_uri_path(uri)
        .filter(|path| path.is_file())
        .map(|path| diagnose_project_source(&path, source))
        .unwrap_or_else(|| diagnose_source(source, uri))
        .into_iter()
        .map(|diagnostic| {
            let severity = match diagnostic.severity {
                DiagnosticSeverity::Error => 1,
                DiagnosticSeverity::Warning => 2,
                DiagnosticSeverity::Information => 3,
            };
            json!({"range":diagnostic.range,"severity":severity,"code":diagnostic.code,
                "source":"glyph","message":diagnostic.message})
        })
        .collect::<Vec<_>>();
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
        "params":{"uri":uri,"diagnostics":diagnostics}})
}
fn completion_result(source: &str, params: &Value) -> Value {
    let position = &params["position"];
    let line = position["line"].as_u64().unwrap_or(0) as u32;
    let character = position["character"].as_u64().unwrap_or(0) as u32;
    let items = complete_source(source, TextPosition { line, character })
        .into_iter()
        .map(|item| {
            let kind = match item.kind.as_str() {
                "property" => 10,
                "keyword" => 14,
                "reference" => 18,
                _ => 1,
            };
            json!({"label":item.label,"kind":kind,"detail":item.detail,
                "insertText":item.insert_text,"insertTextFormat":1})
        })
        .collect::<Vec<_>>();
    json!({"isIncomplete":false,"items":items})
}

fn text_document_uri(params: &Value) -> Option<&str> {
    params.get("textDocument")?.get("uri")?.as_str()
}

fn position_from_json(value: &Value) -> TextPosition {
    TextPosition {
        line: value["line"].as_u64().unwrap_or(0) as u32,
        character: value["character"].as_u64().unwrap_or(0) as u32,
    }
}

fn range_from_json(value: &Value) -> TextRange {
    TextRange {
        start: position_from_json(&value["start"]),
        end: position_from_json(&value["end"]),
    }
}

pub(crate) fn apply_content_changes(source: &str, changes: &[Value]) -> Result<String> {
    let mut current = source.to_string();
    for change in changes {
        let replacement = change
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(range) = change.get("range") else {
            current = replacement.to_string();
            continue;
        };
        let range = range_from_json(range);
        let start = byte_offset_at_position(&current, range.start);
        let end = byte_offset_at_position(&current, range.end);
        if start > end || !current.is_char_boundary(start) || !current.is_char_boundary(end) {
            return Err(anyhow!("invalid incremental text change range"));
        }
        current.replace_range(start..end, replacement);
    }
    Ok(current)
}

fn workspace_edit(uri: &str, edits: impl IntoIterator<Item = (TextRange, String)>) -> Value {
    let edits = edits
        .into_iter()
        .map(|(range, new_text)| json!({"range":range,"newText":new_text}))
        .collect::<Vec<_>>();
    let mut changes = serde_json::Map::new();
    changes.insert(uri.to_string(), Value::Array(edits));
    json!({"changes":Value::Object(changes)})
}

fn file_uri(path: &std::path::Path) -> String {
    let mut encoded = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn document_overlays(documents: &BTreeMap<String, String>) -> BTreeMap<PathBuf, String> {
    documents
        .iter()
        .filter_map(|(uri, source)| {
            let path = file_uri_path(uri)?;
            let canonical = path.canonicalize().ok()?;
            Some((canonical, source.clone()))
        })
        .collect()
}

fn project_workspace_edit(edits: Vec<crate::language_service::ProjectSourceTextEdit>) -> Value {
    let mut grouped = BTreeMap::<String, Vec<Value>>::new();
    for edit in edits {
        grouped
            .entry(file_uri(std::path::Path::new(&edit.path)))
            .or_default()
            .push(json!({"range":edit.range,"newText":edit.new_text}));
    }
    let changes = grouped
        .into_iter()
        .map(|(uri, edits)| (uri, Value::Array(edits)))
        .collect::<serde_json::Map<_, _>>();
    json!({"changes":Value::Object(changes)})
}

fn diagnostics_from_code_action(params: &Value) -> Vec<LanguageDiagnostic> {
    params["context"]["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| LanguageDiagnostic {
            severity: DiagnosticSeverity::Information,
            code: item["code"].as_str().unwrap_or_default().to_string(),
            message: item["message"].as_str().unwrap_or_default().to_string(),
            range: range_from_json(&item["range"]),
        })
        .collect()
}

fn range_contains(range: TextRange, position: TextPosition) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn definition_result(
    documents: &BTreeMap<String, String>,
    uri: &str,
    position: TextPosition,
) -> Result<Value, String> {
    let source = documents
        .get(uri)
        .ok_or_else(|| "document is not open".to_string())?;
    if let Some(reference) = definition_at(source, position) {
        return Ok(json!({"uri":uri,"range":reference.range}));
    }
    let Some(path) = file_uri_path(uri) else {
        return Ok(Value::Null);
    };
    let overlays = document_overlays(documents);
    Ok(project_definition_at(&path, source, position, &overlays)?
        .map(|location| json!({"uri":file_uri(std::path::Path::new(&location.path)),"range":location.range}))
        .unwrap_or(Value::Null))
}

fn references_result(
    documents: &BTreeMap<String, String>,
    uri: &str,
    position: TextPosition,
    include_declaration: bool,
) -> Result<Value, String> {
    let source = documents
        .get(uri)
        .ok_or_else(|| "document is not open".to_string())?;
    if node_symbol_at_position(source, position).is_some() {
        let references = references_at(source, position, include_declaration)
            .into_iter()
            .map(|reference| json!({"uri":uri,"range":reference.range}))
            .collect::<Vec<_>>();
        return Ok(Value::Array(references));
    }
    let Some(path) = file_uri_path(uri) else {
        return Ok(Value::Array(vec![]));
    };
    let overlays = document_overlays(documents);
    let references = project_references_at(&path, source, position, include_declaration, &overlays)?
        .into_iter()
        .map(|location| {
            json!({"uri":file_uri(std::path::Path::new(&location.path)),"range":location.range})
        })
        .collect::<Vec<_>>();
    Ok(Value::Array(references))
}

fn prepare_rename_result(
    documents: &BTreeMap<String, String>,
    uri: &str,
    position: TextPosition,
) -> Result<Value, String> {
    let source = documents
        .get(uri)
        .ok_or_else(|| "document is not open".to_string())?;
    if let Some((symbol, range)) = node_symbol_at_position(source, position) {
        return Ok(json!({"range":range,"placeholder":symbol}));
    }
    let Some(path) = file_uri_path(uri) else {
        return Ok(Value::Null);
    };
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("resolve Glyph source {}: {error}", path.display()))?;
    let overlays = document_overlays(documents);
    let references = project_references_at(&path, source, position, true, &overlays)?;
    Ok(references
        .into_iter()
        .find(|location| {
            std::path::Path::new(&location.path) == canonical
                && range_contains(location.range, position)
        })
        .map(|location| json!({"range":location.range,"placeholder":location.name}))
        .unwrap_or(Value::Null))
}

fn rename_result(
    documents: &BTreeMap<String, String>,
    uri: &str,
    position: TextPosition,
    new_name: &str,
) -> Result<Value, String> {
    let source = documents
        .get(uri)
        .ok_or_else(|| "document is not open".to_string())?;
    if node_symbol_at_position(source, position).is_some() {
        let plan = rename_node_at(source, position, new_name)?;
        return Ok(workspace_edit(
            uri,
            plan.edits
                .into_iter()
                .map(|edit| (edit.range, edit.new_text)),
        ));
    }
    let path = file_uri_path(uri).ok_or_else(|| "rename requires a file URI".to_string())?;
    let overlays = document_overlays(documents);
    let plan = rename_project_symbol_at(&path, source, position, new_name, &overlays)?;
    Ok(project_workspace_edit(plan.edits))
}

fn hover_result(
    documents: &BTreeMap<String, String>,
    uri: &str,
    position: TextPosition,
) -> Result<Value, String> {
    let source = documents
        .get(uri)
        .ok_or_else(|| "document is not open".to_string())?;
    let hover = if let Some(local) = crate::language_service::hover_at(source, position) {
        Some(local)
    } else if let Some(path) = file_uri_path(uri).filter(|path| path.exists()) {
        project_hover_at(&path, source, position, &document_overlays(documents))?
    } else {
        None
    };
    Ok(hover
        .map(|hover| {
            json!({
                "contents":{"kind":"markdown","value":hover.contents},
                "range":hover.range
            })
        })
        .unwrap_or(Value::Null))
}

fn semantic_token_type(kind: SemanticTokenKind) -> u32 {
    match kind {
        SemanticTokenKind::Keyword => 0,
        SemanticTokenKind::Class => 1,
        SemanticTokenKind::Type => 2,
        SemanticTokenKind::Variable => 3,
        SemanticTokenKind::Property => 4,
        SemanticTokenKind::Parameter => 5,
        SemanticTokenKind::Namespace => 6,
    }
}

fn semantic_tokens_result(source: &str) -> Value {
    let mut previous_line = 0u32;
    let mut previous_start = 0u32;
    let mut data = Vec::<u32>::new();
    for token in semantic_tokens(source) {
        let line = token.range.start.line;
        let start = token.range.start.character;
        let delta_line = line - previous_line;
        let delta_start = if delta_line == 0 {
            start - previous_start
        } else {
            start
        };
        let length = token
            .range
            .end
            .character
            .saturating_sub(token.range.start.character);
        if length == 0 || token.range.end.line != line {
            continue;
        }
        data.extend([
            delta_line,
            delta_start,
            length,
            semantic_token_type(token.kind),
            0,
        ]);
        previous_line = line;
        previous_start = start;
    }
    json!({"data":data})
}

pub fn run_stdio() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut documents = BTreeMap::<String, String>::new();
    let mut shutdown = false;
    while let Some(message) = read_message(&mut reader)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => {
                if let Some(id) = id {
                    write_message(
                        &mut writer,
                        &response(
                            id,
                            json!({
                                "capabilities": {
                                    "textDocumentSync": 2,
                                    "completionProvider": {"resolveProvider":false,"triggerCharacters":["."]},
                                    "definitionProvider": true,
                                    "referencesProvider": true,
                                    "hoverProvider": true,
                                    "semanticTokensProvider": {
                                        "legend": {
                                            "tokenTypes":["keyword","class","type","variable","property","parameter","namespace"],
                                            "tokenModifiers":[]
                                        },
                                        "full": true
                                    },
                                    "renameProvider": {"prepareProvider":true},
                                    "codeActionProvider": {"codeActionKinds":["quickfix"]}
                                },
                                "serverInfo":{"name":"glyph-lsp","version":env!("CARGO_PKG_VERSION")}
                            }),
                        ),
                    )?;
                }
            }
            "shutdown" => {
                shutdown = true;
                if let Some(id) = id {
                    write_message(&mut writer, &response(id, Value::Null))?;
                }
            }
            "exit" => break,
            "textDocument/didOpen" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let text = params["textDocument"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                documents.insert(uri.clone(), text.clone());
                write_message(&mut writer, &diagnostic_notification(&uri, &text))?;
            }
            "textDocument/didChange" => {
                let Some(uri) = text_document_uri(&params).map(str::to_string) else {
                    continue;
                };
                let changes = params["contentChanges"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let current = documents.get(&uri).cloned().unwrap_or_default();
                match apply_content_changes(&current, &changes) {
                    Ok(text) => {
                        documents.insert(uri.clone(), text.clone());
                        write_message(&mut writer, &diagnostic_notification(&uri, &text))?;
                    }
                    Err(error) => {
                        write_message(
                            &mut writer,
                            &json!({"jsonrpc":"2.0","method":"window/logMessage",
                                "params":{"type":1,"message":format!("Glyph incremental sync rejected: {error}")}}),
                        )?;
                    }
                }
            }
            "textDocument/didClose" => {
                if let Some(uri) = text_document_uri(&params) {
                    documents.remove(uri);
                    write_message(
                        &mut writer,
                        &json!({
                            "jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
                            "params":{"uri":uri,"diagnostics":[]}
                        }),
                    )?;
                }
            }
            "textDocument/completion" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let result = documents
                        .get(uri)
                        .map(|source| completion_result(source, &params))
                        .unwrap_or_else(|| json!({"isIncomplete":false,"items":[]}));
                    write_message(&mut writer, &response(id, result))?;
                }
            }
            "textDocument/hover" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let position = position_from_json(&params["position"]);
                    match hover_result(&documents, uri, position) {
                        Ok(result) => write_message(&mut writer, &response(id, result))?,
                        Err(message) => {
                            write_message(&mut writer, &error_response(id, -32602, &message))?
                        }
                    }
                }
            }
            "textDocument/semanticTokens/full" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let result = documents
                        .get(uri)
                        .map(|source| semantic_tokens_result(source))
                        .unwrap_or_else(|| json!({"data":[]}));
                    write_message(&mut writer, &response(id, result))?;
                }
            }
            "textDocument/definition" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let position = position_from_json(&params["position"]);
                    match definition_result(&documents, uri, position) {
                        Ok(result) => write_message(&mut writer, &response(id, result))?,
                        Err(message) => {
                            write_message(&mut writer, &error_response(id, -32602, &message))?
                        }
                    }
                }
            }
            "textDocument/references" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let position = position_from_json(&params["position"]);
                    let include_declaration = params["context"]["includeDeclaration"]
                        .as_bool()
                        .unwrap_or(true);
                    match references_result(&documents, uri, position, include_declaration) {
                        Ok(result) => write_message(&mut writer, &response(id, result))?,
                        Err(message) => {
                            write_message(&mut writer, &error_response(id, -32602, &message))?
                        }
                    }
                }
            }
            "textDocument/prepareRename" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let position = position_from_json(&params["position"]);
                    match prepare_rename_result(&documents, uri, position) {
                        Ok(result) => write_message(&mut writer, &response(id, result))?,
                        Err(message) => {
                            write_message(&mut writer, &error_response(id, -32602, &message))?
                        }
                    }
                }
            }
            "textDocument/rename" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let position = position_from_json(&params["position"]);
                    let new_name = params["newName"].as_str().unwrap_or_default();
                    match rename_result(&documents, uri, position, new_name) {
                        Ok(edit) => write_message(&mut writer, &response(id, edit))?,
                        Err(message) => {
                            write_message(&mut writer, &error_response(id, -32602, &message))?
                        }
                    }
                }
            }
            "textDocument/codeAction" => {
                if let Some(id) = id {
                    let uri = text_document_uri(&params).unwrap_or_default();
                    let diagnostics = diagnostics_from_code_action(&params);
                    let result = documents
                        .get(uri)
                        .map(|source| {
                            code_actions(source, &diagnostics)
                                .into_iter()
                                .map(|action| {
                                    let edit = workspace_edit(
                                        uri,
                                        action
                                            .edits
                                            .into_iter()
                                            .map(|edit| (edit.range, edit.new_text)),
                                    );
                                    json!({"title":action.title,"kind":action.kind,"edit":edit})
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    write_message(&mut writer, &response(id, Value::Array(result)))?;
                }
            }
            _ => {
                if let Some(id) = id {
                    write_message(&mut writer, &error_response(id, -32601, "method not found"))?;
                }
            }
        }
        if shutdown && method == "exit" {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_changes_apply_sequentially_with_utf16_positions() {
        let source = "scene \"Sync\" {\n  text title { content \"A😀B\"; }\n}\n";
        let changes = vec![
            json!({
                "range": {"start":{"line":1,"character":25},"end":{"line":1,"character":27}},
                "text":"X"
            }),
            json!({
                "range": {"start":{"line":1,"character":26},"end":{"line":1,"character":27}},
                "text":"Y"
            }),
        ];
        let edited = apply_content_changes(source, &changes).unwrap();
        assert!(edited.contains("\"AXY\""), "{edited}");
    }

    #[test]
    fn full_document_change_still_replaces_buffer() {
        let changes = vec![json!({"text":"scene \"New\" { canvas 10 10; }"})];
        assert_eq!(
            apply_content_changes("scene \"Old\" { canvas 1 1; }", &changes).unwrap(),
            "scene \"New\" { canvas 10 10; }"
        );
    }

    #[test]
    fn project_lsp_navigation_and_rename_return_cross_file_workspace_edits() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("glyph.project.json"), r#"{"version":1}"#).unwrap();
        let library = dir.path().join("library.glyph");
        let main = dir.path().join("main.glyph");
        let library_source = r#"scene "Library" {
  canvas 100 100;
  component badge { rect bg { x 0; y 0; width 40; height 20; } }
}"#;
        let main_source = r#"import "library.glyph" as ui;
scene "Main" {
  canvas 200 120;
  instance ui/badge card { x 20; y 20; }
}"#;
        fs::write(&library, library_source).unwrap();
        fs::write(&main, main_source).unwrap();
        let library = library.canonicalize().unwrap();
        let main = main.canonicalize().unwrap();
        let library_uri = file_uri(&library);
        let main_uri = file_uri(&main);
        let documents = BTreeMap::from([
            (library_uri.clone(), library_source.to_string()),
            (main_uri.clone(), main_source.to_string()),
        ]);
        let position = TextPosition {
            line: 3,
            character: 11,
        };

        let definition = definition_result(&documents, &main_uri, position).unwrap();
        assert_eq!(definition["uri"], library_uri);
        assert_eq!(definition["range"]["start"]["line"], 2);

        let references = references_result(&documents, &main_uri, position, true).unwrap();
        let refs = references.as_array().unwrap();
        assert_eq!(refs.len(), 2, "{references}");
        assert!(refs.iter().any(|reference| reference["uri"] == library_uri));
        assert!(refs.iter().any(|reference| reference["uri"] == main_uri));

        let prepared = prepare_rename_result(&documents, &main_uri, position).unwrap();
        assert_eq!(prepared["placeholder"], "ui/badge");

        let rename = rename_result(&documents, &main_uri, position, "chip").unwrap();
        let changes = rename["changes"].as_object().unwrap();
        assert_eq!(changes.len(), 2, "{rename}");
        assert_eq!(changes[&library_uri][0]["newText"], "chip");
        assert_eq!(changes[&main_uri][0]["newText"], "ui/chip");

        let hover = hover_result(&documents, &main_uri, position).unwrap();
        assert!(
            hover["contents"]["value"]
                .as_str()
                .unwrap()
                .contains("**component** `ui/badge`"),
            "{hover}"
        );
        let semantic = semantic_tokens_result(main_source);
        assert!(
            semantic["data"]
                .as_array()
                .is_some_and(|data| !data.is_empty()),
            "{semantic}"
        );
    }
}
