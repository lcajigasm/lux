use std::path::PathBuf;

use crate::dialog::{FileDialogOptions, FileDialogs};

pub(super) struct RfdFileDialogs;

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
