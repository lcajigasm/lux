//! Dialog overlays grouping (ST-FIX-02).

#[derive(Debug, Default)]
pub(crate) struct DialogState {
    pub(crate) conflict_prompt: Option<crate::conflict::ConflictPrompt>,
    pub(crate) about: Option<crate::about::AboutDialogState>,
    /// Dialogs and file I/O running on workers.
    pub(crate) jobs: crate::ui_io::Jobs,
}
