//! Extensible Sublime Text Snippet Engine (ST18).

pub mod builtins;
pub mod dispatch;
pub mod parser;
pub mod picker;
pub mod registry;

pub(crate) use picker::{SnippetPickerState, handle_snippet_picker_key};
#[allow(unused_imports)]
pub(crate) use registry::{Snippet, SnippetRegistry};

/// Grouped snippet state: registry of snippets plus active modal picker.
pub(crate) struct SnippetState {
    pub(crate) registry: SnippetRegistry,
    pub(crate) picker: Option<SnippetPickerState>,
}

impl Default for SnippetState {
    fn default() -> Self {
        Self::new()
    }
}

impl SnippetState {
    pub(crate) fn new() -> Self {
        let mut registry = SnippetRegistry::new();
        registry.load_custom_snippets(None);
        Self {
            registry,
            picker: None,
        }
    }
}
