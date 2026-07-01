//! The fixed M8 base capability set (spec ch. 16 §16.1, KDR-0103). The server
//! advertises exactly these capabilities on `initialize` regardless of client
//! capabilities — there is no negotiation surface in the M8 base server.

use lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind,
};

#[must_use]
pub fn initialize_result() -> InitializeResult {
    InitializeResult {
        capabilities: ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL,
            )),
            definition_provider: Some(OneOf::Left(true)),
            completion_provider: Some(CompletionOptions {
                resolve_provider: Some(false),
                trigger_characters: Some(Vec::new()),
                ..Default::default()
            }),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            ..Default::default()
        },
        server_info: Some(ServerInfo {
            name: "keel-lsp".to_owned(),
            version: None,
        }),
    }
}
