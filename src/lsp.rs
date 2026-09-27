use crate::language_service::{
    DiagnosticSeverity, TextPosition, complete_source, diagnose_project_source, diagnose_source,
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
                                    "textDocumentSync": 1,
                                    "completionProvider": {"resolveProvider":false,"triggerCharacters":["."]}
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
                let text = params["contentChanges"]
                    .as_array()
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                documents.insert(uri.clone(), text.clone());
                write_message(&mut writer, &diagnostic_notification(&uri, &text))?;
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
