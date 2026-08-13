//! Completion with lazy `resolve`, server-driven sort order, and staleness
//! rejection so a fast typist never sees an old keystroke's results
//! overwrite a newer one's -- the concrete mechanism behind "filtrado
//! incremental sin latencia perceptible".

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    CompletionContext, CompletionItem, CompletionParams, CompletionResponse, CompletionTriggerKind,
    PartialResultParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
    WorkDoneProgressParams,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    #[error("language server is not connected")]
    NotConnected,
    /// A newer [`Completion::complete`] call started before this one's
    /// response arrived. The result is discarded rather than returned, so
    /// a slow, stale response from an earlier keystroke can never overwrite
    /// what a later one produced.
    #[error("a newer completion request superseded this one")]
    Superseded,
    #[error("completion request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

/// Requests completions for one session, keeping only the most recent
/// request's result relevant.
pub struct Completion {
    session: Arc<ServerSession>,
    generation: AtomicU64,
}

impl Completion {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self {
            session,
            generation: AtomicU64::new(0),
        }
    }

    /// Request completions at `position`, sorted the way the server asked
    /// (`sortText`, falling back to `label` when absent, per the LSP spec's
    /// client-sorting contract). See [`CompletionError::Superseded`] for
    /// what happens if a newer call starts before this one resolves.
    pub async fn complete(
        &self,
        uri: &Url,
        position: Position,
        trigger_character: Option<String>,
    ) -> Result<Vec<CompletionItem>, CompletionError> {
        let my_generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut socket = self.session.socket().ok_or(CompletionError::NotConnected)?;

        let context = trigger_character.map(|trigger_character| CompletionContext {
            trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some(trigger_character),
        });
        let response = socket
            .completion(CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
                context,
            })
            .await?;

        if self.generation.load(Ordering::SeqCst) != my_generation {
            return Err(CompletionError::Superseded);
        }

        let mut items = match response {
            Some(CompletionResponse::Array(items)) => items,
            Some(CompletionResponse::List(list)) => list.items,
            None => Vec::new(),
        };
        sort_items(&mut items);
        Ok(items)
    }

    /// Lazily resolve additional detail (typically documentation) for one
    /// item. Returns `item` unchanged when the server didn't advertise
    /// `completionProvider.resolveProvider` -- not every server supports
    /// resolve, and asking one that doesn't would just be a wasted request.
    pub async fn resolve(&self, item: CompletionItem) -> Result<CompletionItem, CompletionError> {
        if !self.resolve_supported() {
            return Ok(item);
        }
        let mut socket = self.session.socket().ok_or(CompletionError::NotConnected)?;
        Ok(socket.completion_item_resolve(item).await?)
    }

    fn resolve_supported(&self) -> bool {
        self.session
            .watch()
            .borrow()
            .capabilities
            .as_ref()
            .and_then(|c| c.completion_provider.as_ref())
            .and_then(|cp| cp.resolve_provider)
            .unwrap_or(false)
    }
}

/// The server picks the intended order via `sortText`; a missing
/// `sortText` falls back to the item's own label, per spec.
fn sort_key(item: &CompletionItem) -> &str {
    item.sort_text.as_deref().unwrap_or(&item.label)
}

fn sort_items(items: &mut [CompletionItem]) {
    items.sort_by(|a, b| sort_key(a).cmp(sort_key(b)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, sort_text: Option<&str>) -> CompletionItem {
        CompletionItem {
            label: label.into(),
            sort_text: sort_text.map(String::from),
            ..Default::default()
        }
    }

    #[test]
    fn sorts_by_sort_text_when_present() {
        let mut items = vec![
            item("zzz", Some("1")),
            item("aaa", Some("2")),
            item("mmm", Some("0")),
        ];
        sort_items(&mut items);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["mmm", "zzz", "aaa"]);
    }

    #[test]
    fn falls_back_to_label_when_sort_text_is_absent() {
        let mut items = vec![
            item("charlie", None),
            item("alpha", None),
            item("bravo", None),
        ];
        sort_items(&mut items);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn mixes_sort_text_and_label_fallback_correctly() {
        // An item with an explicit `sortText` of "0" should sort before a
        // label-only item whose label starts with a later character, even
        // though comparing raw labels alone would disagree.
        let mut items = vec![item("zzz", Some("0")), item("aaa", None)];
        sort_items(&mut items);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["zzz", "aaa"]);
    }
}

#[cfg(test)]
mod real_server;
