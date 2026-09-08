use std::path::PathBuf;

use crate::dialog::{FileDialogOptions, FileDialogs, SaveDecision};

#[derive(Default)]
pub(super) struct RfdFileDialogs {
    confirmation: std::sync::Mutex<rfd::MessageDialog>,
}

impl RfdFileDialogs {
    pub(super) fn with_parent(window: &winit::window::Window) -> Self {
        Self {
            confirmation: std::sync::Mutex::new(rfd::MessageDialog::new().set_parent(window)),
        }
    }
}

fn build(options: &FileDialogOptions) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = &options.title {
        dialog = dialog.set_title(title.as_str());
    }
    if let Some(dir) = &options.starting_directory {
        dialog = dialog.set_directory(dir);
    }
    for (name, extensions) in &options.filters {
        dialog = dialog.add_filter(name.as_str(), extensions);
    }
    dialog
}

impl FileDialogs for RfdFileDialogs {
    fn confirm_save(&self, document: &str) -> SaveDecision {
        let dialog = self
            .confirmation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        match dialog
            .set_title("Unsaved changes")
            .set_description(format!("Save changes to {document} before continuing?"))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => SaveDecision::Save,
            rfd::MessageDialogResult::No => SaveDecision::Discard,
            _ => SaveDecision::Cancel,
        }
    }
    fn pick_file(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        build(options).pick_file()
    }

    fn pick_files(&self, options: &FileDialogOptions) -> Vec<PathBuf> {
        build(options).pick_files().unwrap_or_default()
    }

    fn pick_folder(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        build(options).pick_folder()
    }

    fn save_file(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        build(options).save_file()
    }
}
