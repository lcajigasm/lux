//! Navigation: go to definition/declaration/type/implementations, find
//! references, call hierarchy, and document/workspace symbols.

use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::ServerSocket;
use async_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Location, OneOf, PartialResultParams, Position, Range,
    ReferenceContext, ReferenceParams, SymbolInformation, SymbolKind, TextDocumentIdentifier,
    TextDocumentPositionParams, Url, WorkDoneProgressParams, WorkspaceSymbolParams,
    WorkspaceSymbolResponse,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum NavigationError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("navigation request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

/// One normalized document-symbol/outline entry: the server's own nested
/// `DocumentSymbol` shape when it provides one, or a childless entry built
/// from the older flat `SymbolInformation` list otherwise.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub range: Range,
    pub selection_range: Range,
    pub children: Vec<SymbolEntry>,
}

/// One normalized `workspace/symbol` match.
#[derive(Debug, Clone)]
pub struct WorkspaceSymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub container_name: Option<String>,
    /// `None` for a 3.17 "partial" workspace symbol (location without a
    /// range) that was never resolved via `workspaceSymbol/resolve` --
    /// out of this deliverable's scope, same as the servers this crate
    /// targets (rust-analyzer et al.) not sending them by default.
    pub location: Option<Location>,
}

/// Navigation requests for one session.
pub struct Navigation {
    session: Arc<ServerSession>,
}

impl Navigation {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    pub async fn definition(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<Location>, NavigationError> {
        let mut socket = self.socket()?;
        Ok(locations_from_goto(
            socket.definition(goto_params(uri, position)).await?,
        ))
    }

    pub async fn declaration(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<Location>, NavigationError> {
        let mut socket = self.socket()?;
        Ok(locations_from_goto(
            socket.declaration(goto_params(uri, position)).await?,
        ))
    }

    pub async fn type_definition(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<Location>, NavigationError> {
        let mut socket = self.socket()?;
        Ok(locations_from_goto(
            socket.type_definition(goto_params(uri, position)).await?,
        ))
    }

    pub async fn implementations(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<Location>, NavigationError> {
        let mut socket = self.socket()?;
        Ok(locations_from_goto(
            socket.implementation(goto_params(uri, position)).await?,
        ))
    }

    pub async fn references(
        &self,
        uri: &Url,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .references(ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
                context: ReferenceContext {
                    include_declaration,
                },
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// The document's outline, hierarchical when the server provides one.
    pub async fn document_symbols(&self, uri: &Url) -> Result<Vec<SymbolEntry>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .document_symbol(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;
        Ok(match response {
            Some(DocumentSymbolResponse::Nested(symbols)) => {
                symbols.into_iter().map(symbol_entry_from_nested).collect()
            }
            Some(DocumentSymbolResponse::Flat(infos)) => {
                infos.into_iter().map(symbol_entry_from_flat).collect()
            }
            None => Vec::new(),
        })
    }

    /// Workspace-wide symbols matching `query`.
    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<Vec<WorkspaceSymbolEntry>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .symbol(WorkspaceSymbolParams {
                partial_result_params: PartialResultParams::default(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                query: query.to_string(),
            })
            .await?;
        Ok(match response {
            Some(WorkspaceSymbolResponse::Flat(infos)) => infos
                .into_iter()
                .map(|info| WorkspaceSymbolEntry {
                    name: info.name,
                    kind: info.kind,
                    container_name: info.container_name,
                    location: Some(info.location),
                })
                .collect(),
            Some(WorkspaceSymbolResponse::Nested(symbols)) => symbols
                .into_iter()
                .map(|symbol| WorkspaceSymbolEntry {
                    name: symbol.name,
                    kind: symbol.kind,
                    container_name: symbol.container_name,
                    location: match symbol.location {
                        OneOf::Left(location) => Some(location),
                        OneOf::Right(_) => None,
                    },
                })
                .collect(),
            None => Vec::new(),
        })
    }

    /// The symbol(s) at `position`, as call-hierarchy roots -- pass one of
    /// these to [`Self::incoming_calls`]/[`Self::outgoing_calls`].
    pub async fn prepare_call_hierarchy(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Vec<CallHierarchyItem>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .prepare_call_hierarchy(CallHierarchyPrepareParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// Callers of `item`.
    pub async fn incoming_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyIncomingCall>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .incoming_calls(CallHierarchyIncomingCallsParams {
                item,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// Callees of `item`.
    pub async fn outgoing_calls(
        &self,
        item: CallHierarchyItem,
    ) -> Result<Vec<CallHierarchyOutgoingCall>, NavigationError> {
        let mut socket = self.socket()?;
        let response = socket
            .outgoing_calls(CallHierarchyOutgoingCallsParams {
                item,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    fn socket(&self) -> Result<ServerSocket, NavigationError> {
        self.session.socket().ok_or(NavigationError::NotConnected)
    }
}

fn goto_params(uri: &Url, position: Position) -> GotoDefinitionParams {
    GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    }
}

/// `textDocument/{definition,declaration,typeDefinition,implementation}`
/// all share this response shape (`GotoDeclarationResponse` etc. are type
/// aliases of it in `lsp_types`), so one normalizer covers all four.
fn locations_from_goto(response: Option<GotoDefinitionResponse>) -> Vec<Location> {
    match response {
        Some(GotoDefinitionResponse::Scalar(location)) => vec![location],
        Some(GotoDefinitionResponse::Array(locations)) => locations,
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .map(|link| Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect(),
        None => Vec::new(),
    }
}

fn symbol_entry_from_nested(symbol: DocumentSymbol) -> SymbolEntry {
    SymbolEntry {
        name: symbol.name,
        kind: symbol.kind,
        range: symbol.range,
        selection_range: symbol.selection_range,
        children: symbol
            .children
            .unwrap_or_default()
            .into_iter()
            .map(symbol_entry_from_nested)
            .collect(),
    }
}

fn symbol_entry_from_flat(info: SymbolInformation) -> SymbolEntry {
    SymbolEntry {
        name: info.name,
        kind: info.kind,
        range: info.location.range,
        selection_range: info.location.range,
        children: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn location(line: u32) -> Location {
        Location {
            uri: Url::parse("file:///a.rs").unwrap(),
            range: Range {
                start: pos(line, 0),
                end: pos(line, 5),
            },
        }
    }

    #[test]
    fn goto_response_scalar_becomes_one_location() {
        let locations = locations_from_goto(Some(GotoDefinitionResponse::Scalar(location(1))));
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start.line, 1);
    }

    #[test]
    fn goto_response_array_passes_through() {
        let locations = locations_from_goto(Some(GotoDefinitionResponse::Array(vec![
            location(1),
            location(2),
        ])));
        assert_eq!(
            locations
                .iter()
                .map(|l| l.range.start.line)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn goto_response_link_uses_the_target_selection_range() {
        use async_lsp::lsp_types::LocationLink;
        let link = LocationLink {
            origin_selection_range: None,
            target_uri: Url::parse("file:///b.rs").unwrap(),
            target_range: Range {
                start: pos(0, 0),
                end: pos(10, 0),
            },
            target_selection_range: Range {
                start: pos(3, 4),
                end: pos(3, 8),
            },
        };
        let locations = locations_from_goto(Some(GotoDefinitionResponse::Link(vec![link])));
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].range.start, pos(3, 4));
    }

    #[test]
    fn goto_response_none_is_an_empty_list() {
        assert!(locations_from_goto(None).is_empty());
    }

    fn nested_symbol(name: &str, children: Vec<DocumentSymbol>) -> DocumentSymbol {
        #[allow(deprecated)]
        DocumentSymbol {
            name: name.into(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range: Range {
                start: pos(0, 0),
                end: pos(0, 0),
            },
            selection_range: Range {
                start: pos(0, 0),
                end: pos(0, 0),
            },
            children: (!children.is_empty()).then_some(children),
        }
    }

    #[test]
    fn nested_document_symbols_keep_their_children() {
        let root = nested_symbol("Outer", vec![nested_symbol("inner", vec![])]);
        let entries: Vec<SymbolEntry> = vec![root]
            .into_iter()
            .map(symbol_entry_from_nested)
            .collect();
        assert_eq!(entries[0].name, "Outer");
        assert_eq!(entries[0].children.len(), 1);
        assert_eq!(entries[0].children[0].name, "inner");
    }

    #[test]
    fn flat_symbol_information_becomes_a_childless_entry() {
        #[allow(deprecated)]
        let info = SymbolInformation {
            name: "flat".into(),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            location: location(7),
            container_name: None,
        };
        let entry = symbol_entry_from_flat(info);
        assert_eq!(entry.name, "flat");
        assert!(entry.children.is_empty());
        assert_eq!(entry.range.start.line, 7);
    }
}

#[cfg(test)]
mod real_server;
