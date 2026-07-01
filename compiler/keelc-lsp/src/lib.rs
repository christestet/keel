//! M8 base LSP server (spec ch. 16, KDR-0103): `keel lsp`'s protocol handlers
//! and workspace state.
//!
//! `serve` runs a single-threaded, explicit dispatch loop over any
//! `BufRead`/`Write` pair (real process stdio in production, in-memory
//! buffers in `tests/transcripts.rs`). It reads frames with
//! `lsp_server::Message::read` rather than `lsp_server::Connection::stdio`'s
//! threaded transport, because that transport has no way to recover from a
//! single malformed frame — its reader thread just exits, silently dropping
//! the connection. Reading frames directly lets a JSON parse error produce
//! the `-32700` response spec ch. 16 §16.4 requires and keep serving
//! subsequent, well-formed frames.

mod capabilities;
mod diagnostics;
mod documents;
mod frame;
mod symbols;

use documents::{Document, Utf16Index};
use keelc_ast::{Item, Module};
use lsp_server::{ErrorCode, Message, Notification, Request};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentSymbol, DocumentSymbolParams,
    Hover, HoverContents, Location, MarkupContent, MarkupKind, PublishDiagnosticsParams,
    SymbolKind, TextDocumentPositionParams, Uri,
};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

/// Runs the dispatch loop until `exit` or end of input. `milestone` is the
/// Core milestone every document is checked against (`keel lsp` always passes
/// the highest implemented milestone; see `keelc-driver::LSP_MILESTONE`).
pub fn serve(reader: &mut impl BufRead, writer: &mut impl Write, milestone: u32) -> io::Result<()> {
    let mut server = Server::new(milestone);
    loop {
        match Message::read(reader) {
            Ok(None) => return Ok(()),
            Ok(Some(Message::Request(request))) => {
                let response = server.handle_request(request);
                frame::write_frame(writer, &response)?;
            }
            Ok(Some(Message::Notification(notification))) => {
                let is_exit = notification.method == "exit";
                for outgoing in server.handle_notification(notification) {
                    frame::write_frame(writer, &outgoing)?;
                }
                if is_exit {
                    return Ok(());
                }
            }
            Ok(Some(Message::Response(_))) => {
                // The M8 base server never sends client-bound requests, so it
                // never expects a response back from the client.
            }
            Err(_) => {
                frame::write_frame(
                    writer,
                    &frame::error_response(Value::Null, -32700, "Parse error"),
                )?;
            }
        }
    }
}

struct Server {
    db: keelc_query::QueryDatabase,
    milestone: u32,
    documents: HashMap<Uri, Document>,
}

impl Server {
    fn new(milestone: u32) -> Self {
        Self {
            db: keelc_query::QueryDatabase::default(),
            milestone,
            documents: HashMap::new(),
        }
    }

    fn handle_request(&mut self, request: Request) -> Value {
        let id = serde_json::to_value(&request.id).unwrap_or(Value::Null);
        match request.method.as_str() {
            "initialize" => frame::ok_response(id, capabilities::initialize_result()),
            "shutdown" => frame::ok_response(id, Value::Null),
            "textDocument/definition" => self.definition(id, request.params),
            "textDocument/hover" => self.hover(id, request.params),
            "textDocument/completion" => self.completion(id, request.params),
            "textDocument/documentSymbol" => self.document_symbol(id, request.params),
            _ => frame::error_response(id, ErrorCode::MethodNotFound as i32, "Method not found"),
        }
    }

    fn handle_notification(&mut self, notification: Notification) -> Vec<Value> {
        match notification.method.as_str() {
            "textDocument/didOpen" => self.did_open(notification.params),
            "textDocument/didChange" => self.did_change(notification.params),
            "textDocument/didClose" => self.did_close(notification.params),
            _ => Vec::new(),
        }
    }

    fn did_open(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidOpenTextDocumentParams>(params) else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        self.documents.insert(
            uri.clone(),
            Document {
                text: params.text_document.text,
                version: params.text_document.version,
            },
        );
        vec![self.publish_diagnostics(&uri)]
    }

    fn did_change(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidChangeTextDocumentParams>(params) else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        let Some(document) = self.documents.get_mut(&uri) else {
            return Vec::new();
        };
        for change in &params.content_changes {
            document.text = documents::apply_change(&document.text, change);
        }
        document.version = params.text_document.version;
        vec![self.publish_diagnostics(&uri)]
    }

    fn did_close(&mut self, params: Value) -> Vec<Value> {
        let Ok(params) = serde_json::from_value::<DidCloseTextDocumentParams>(params) else {
            return Vec::new();
        };
        let uri = params.text_document.uri;
        self.documents.remove(&uri);
        vec![frame::notification(
            "textDocument/publishDiagnostics",
            PublishDiagnosticsParams {
                uri,
                diagnostics: Vec::new(),
                version: None,
            },
        )]
    }

    fn publish_diagnostics(&self, uri: &Uri) -> Value {
        let text = self
            .documents
            .get(uri)
            .map_or_else(String::new, |document| document.text.clone());
        let source = keelc_query::SourceFile::new(&self.db, text.clone(), self.milestone);
        let diagnostics = keelc_query::check_diagnostics(&self.db, source);
        frame::notification(
            "textDocument/publishDiagnostics",
            PublishDiagnosticsParams {
                uri: uri.clone(),
                diagnostics: diagnostics::to_lsp_diagnostics(&diagnostics, &text),
                version: None,
            },
        )
    }

    fn definition(&self, id: Value, params: Value) -> Value {
        let Some((uri, text, offset)) = self.position_params(params) else {
            return frame::ok_response(id, Value::Null);
        };
        let Some((name, _)) = symbols::identifier_at(&text, offset) else {
            return frame::ok_response(id, Value::Null);
        };
        let parsed = self.parsed_module(&text);
        let top = symbols::collect(&parsed.module);
        match symbols::find_definition(&top, &name) {
            Some(span) => {
                let index = Utf16Index::new(&text);
                frame::ok_response(
                    id,
                    Location {
                        uri,
                        range: documents::range(&text, &index, span.start, span.end),
                    },
                )
            }
            None => frame::ok_response(id, Value::Null),
        }
    }

    fn hover(&self, id: Value, params: Value) -> Value {
        let Some((_uri, text, offset)) = self.position_params(params) else {
            return frame::ok_response(id, Value::Null);
        };
        let Some((name, span)) = symbols::identifier_at(&text, offset) else {
            return frame::ok_response(id, Value::Null);
        };
        let parsed = self.parsed_module(&text);
        let top = symbols::collect(&parsed.module);
        match symbols::hover_signature(&top, &name) {
            Some(signature) => {
                let index = Utf16Index::new(&text);
                frame::ok_response(
                    id,
                    Hover {
                        contents: HoverContents::Markup(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: format!("```keel\n{signature}\n```"),
                        }),
                        range: Some(documents::range(&text, &index, span.start, span.end)),
                    },
                )
            }
            None => frame::ok_response(id, Value::Null),
        }
    }

    fn completion(&self, id: Value, params: Value) -> Value {
        let Some((_uri, text, offset)) = self.position_params(params) else {
            return frame::ok_response(id, empty_completion_list());
        };
        let prefix = symbols::identifier_at(&text, offset)
            .map(|(name, _)| name)
            .unwrap_or_default();
        let parsed = self.parsed_module(&text);
        let top = symbols::collect(&parsed.module);
        let items = symbols::completions(&top, &prefix)
            .into_iter()
            .map(|candidate| CompletionItem {
                label: candidate.label,
                kind: Some(if candidate.is_function {
                    CompletionItemKind::FUNCTION
                } else {
                    CompletionItemKind::CLASS
                }),
                detail: Some(candidate.detail),
                ..Default::default()
            })
            .collect();
        frame::ok_response(
            id,
            CompletionList {
                is_incomplete: false,
                items,
            },
        )
    }

    fn document_symbol(&self, id: Value, params: Value) -> Value {
        let Ok(params) = serde_json::from_value::<DocumentSymbolParams>(params) else {
            return frame::ok_response(id, Value::Null);
        };
        let Some(document) = self.documents.get(&params.text_document.uri) else {
            return frame::ok_response(id, Vec::<DocumentSymbol>::new());
        };
        let text = document.text.clone();
        let parsed = self.parsed_module(&text);
        let index = Utf16Index::new(&text);
        frame::ok_response(id, document_symbols(&parsed.module, &text, &index))
    }

    fn parsed_module(&self, text: &str) -> std::sync::Arc<keelc_parse::ParseOutput> {
        let source = keelc_query::SourceFile::new(&self.db, text.to_owned(), self.milestone);
        keelc_query::parsed_module(&self.db, source)
    }

    /// Extracts `(uri, current document text, byte offset)` from a
    /// `TextDocumentPositionParams`-shaped request. Definition, hover, and
    /// completion requests all carry this shape (completion's wider
    /// `CompletionParams` still deserializes into it, ignoring the extra
    /// `context` field it adds).
    fn position_params(&self, params: Value) -> Option<(Uri, String, usize)> {
        let params = serde_json::from_value::<TextDocumentPositionParams>(params).ok()?;
        let document = self.documents.get(&params.text_document.uri)?;
        let index = Utf16Index::new(&document.text);
        let offset = index.byte_offset(&document.text, params.position);
        Some((
            params.text_document.uri.clone(),
            document.text.clone(),
            offset,
        ))
    }
}

fn empty_completion_list() -> CompletionList {
    CompletionList {
        is_incomplete: false,
        items: Vec::new(),
    }
}

/// Module-level struct/function outline in source declaration order (spec ch.
/// 16 §16.1 `textDocument/documentSymbol`).
#[allow(deprecated)] // `DocumentSymbol::deprecated` has no replacement field to set instead.
fn document_symbols(module: &Module, text: &str, index: &Utf16Index) -> Vec<DocumentSymbol> {
    module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(decl) => Some(DocumentSymbol {
                name: decl.name.value.clone(),
                detail: None,
                kind: SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                range: documents::range(
                    text,
                    index,
                    decl.span.start,
                    struct_end(text, decl.span.end),
                ),
                selection_range: documents::range(
                    text,
                    index,
                    decl.name.span.start,
                    decl.name.span.end,
                ),
                children: Some(
                    decl.fields
                        .iter()
                        .map(|field| DocumentSymbol {
                            name: field.name.value.clone(),
                            detail: None,
                            kind: SymbolKind::FIELD,
                            tags: None,
                            deprecated: None,
                            range: documents::range(text, index, field.span.start, field.span.end),
                            selection_range: documents::range(
                                text,
                                index,
                                field.name.span.start,
                                field.name.span.end,
                            ),
                            children: None,
                        })
                        .collect(),
                ),
            }),
            Item::Function(decl) => Some(DocumentSymbol {
                name: decl.name.value.clone(),
                detail: None,
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range: documents::range(text, index, decl.span.start, decl.span.end),
                selection_range: documents::range(
                    text,
                    index,
                    decl.name.span.start,
                    decl.name.span.end,
                ),
                children: Some(Vec::new()),
            }),
            _ => None,
        })
        .collect()
}

/// `StructDecl::span` ends at the last field (unlike `FunctionDecl::span`,
/// which extends through its body's closing brace) because
/// `keelc-parse::parse_struct` builds the span from `fields.last()` rather
/// than the brace `parse_braced_fields` already consumed. Diagnostics only
/// read a span's *start*, so that gap is invisible there, but
/// `documentSymbol`'s `range` must enclose the whole declaration. Scanning
/// forward for the `}` the parser already required avoids widening
/// `keelc-parse`'s span (and its diagnostic-rendering guarantees) just for
/// this one LSP-only presentation need.
fn struct_end(text: &str, after: usize) -> usize {
    let after = after.min(text.len());
    text[after..]
        .find('}')
        .map_or(after, |offset| after + offset + 1)
}
