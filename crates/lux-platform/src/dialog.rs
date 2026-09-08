//! Native file/folder picker dialogs.

use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileDialogOptions {
    pub title: Option<String>,
    pub starting_directory: Option<PathBuf>,
    /// Each entry is a filter label paired with the extensions it matches
    /// (without the leading dot), e.g. `("Rust", vec!["rs"])`.
    pub filters: Vec<(String, Vec<String>)>,
}

/// Blocking, native modal dialogs. These show OS UI and wait for the user,
/// same as any other file picker on the platform — call them off the
/// render thread if a caller cannot afford to block a frame on user input.
pub trait FileDialogs: Send + Sync {
    fn pick_file(&self, options: &FileDialogOptions) -> Option<PathBuf>;
    fn pick_files(&self, options: &FileDialogOptions) -> Vec<PathBuf>;
    fn pick_folder(&self, options: &FileDialogOptions) -> Option<PathBuf>;
    fn save_file(&self, options: &FileDialogOptions) -> Option<PathBuf>;
    fn confirm_save(&self, document: &str) -> SaveDecision;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDecision {
    Save,
    Discard,
    Cancel,
}
