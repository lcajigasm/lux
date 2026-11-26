use std::path::PathBuf;

use crate::dialog::{FileDialogOptions, FileDialogs, SaveDecision};

#[derive(Default)]
pub(super) struct RfdFileDialogs {
    confirmation: std::sync::Mutex<rfd::MessageDialog>,
    /// macOS: window-attached sheets for calls from a worker thread. See
    /// `dialog_sheet.rs`; every method falls back to the blocking dialog
    /// when this returns `None`.
    #[cfg(target_os = "macos")]
    sheets: super::dialog_sheet::Sheets,
}

impl RfdFileDialogs {
    pub(super) fn with_parent(window: &winit::window::Window) -> Self {
        Self {
            confirmation: std::sync::Mutex::new(rfd::MessageDialog::new().set_parent(window)),
            #[cfg(target_os = "macos")]
            sheets: super::dialog_sheet::Sheets::with_parent(window),
        }
    }
}

/// Returns early with the sheet's answer when macOS showed one.
macro_rules! sheet_first {
    ($self:ident . $method:ident ( $($arg:expr),* )) => {
        #[cfg(target_os = "macos")]
        if let Some(answer) = $self.sheets.$method($($arg),*) {
            return answer;
        }
    };
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
        sheet_first!(self.confirm_save(document));
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
    fn confirm_trash(&self, item: &str) -> bool {
        sheet_first!(self.confirm_trash(item));
        let dialog = self
            .confirmation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        matches!(
            dialog
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Move to Trash")
                .set_description(format!("Move \"{item}\" to the Trash?"))
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show(),
            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
        )
    }
    fn pick_file(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        sheet_first!(self.pick_file(options));
        build(options).pick_file()
    }

    fn pick_files(&self, options: &FileDialogOptions) -> Vec<PathBuf> {
        sheet_first!(self.pick_files(options));
        build(options).pick_files().unwrap_or_default()
    }

    fn pick_folder(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        sheet_first!(self.pick_folder(options));
        build(options).pick_folder()
    }

    fn save_file(&self, options: &FileDialogOptions) -> Option<PathBuf> {
        sheet_first!(self.save_file(options));
        build(options).save_file()
    }
}
