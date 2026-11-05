//! Background worker document I/O execution. Runs off the main UI thread.

use std::path::PathBuf;

use lux_core::{buffer::Buffer, huge_file::HugeFilePolicy};
use lux_platform::dialog::SaveDecision;
use lux_platform::{FileDialogOptions, FileDialogs};

use super::{NamedSave, Request};

pub(crate) enum Outcome {
    Cancelled,
    Finished {
        saved: Option<(PathBuf, Buffer)>,
        opened: Option<(Option<PathBuf>, Buffer)>,
        close: bool,
        saved_paths: Vec<PathBuf>,
    },
}

pub(crate) struct Snapshot {
    pub(crate) buffer: Buffer,
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    pub(crate) disk: Option<Buffer>,
}

pub(crate) fn save_snapshot(
    snapshot: &Snapshot,
    save_as: bool,
    dialogs: &dyn FileDialogs,
) -> Result<Option<(PathBuf, Buffer)>, String> {
    if snapshot.buffer.is_huge_file_mode() {
        return Err("Large file is read-only".into());
    }
    let path = if save_as || snapshot.path.is_none() {
        dialogs.save_file(&FileDialogOptions {
            title: Some("Save document".into()),
            ..Default::default()
        })
    } else {
        snapshot.path.clone()
    };
    let Some(path) = path else {
        return Ok(None);
    };
    if snapshot.path.as_ref() == Some(&path)
        && let Some(expected) = &snapshot.disk
    {
        let current = Buffer::open(&path)
            .map_err(|e| format!("Cannot check {} before saving: {e}", path.display()))?;
        if current.text() != expected.text() || current.encoding() != expected.encoding() {
            return Err(
                "File changed on disk; save to a different path to preserve both versions".into(),
            );
        }
    }
    snapshot
        .buffer
        .save(&path)
        .map_err(|e| format!("Cannot save {}: {e}", path.display()))?;
    if let Some(store) = recovery_store() {
        let _ = store.clear(&path);
    }
    Ok(Some((path, snapshot.buffer.clone())))
}

pub(crate) fn execute(
    request: Request,
    snapshot: Snapshot,
    dialogs: &dyn FileDialogs,
) -> Result<Outcome, String> {
    if let Request::SaveAll(files) = &request {
        return save_named_files(files, dialogs);
    }
    if let Request::Save { save_as } = request {
        return Ok(match save_snapshot(&snapshot, save_as, dialogs)? {
            Some(saved) => Outcome::Finished {
                saved: Some(saved),
                opened: None,
                close: false,
                saved_paths: Vec::new(),
            },
            None => Outcome::Cancelled,
        });
    }
    let open_path = if let Request::Open(path) = &request {
        let selected = path
            .clone()
            .or_else(|| dialogs.pick_file(&FileDialogOptions::default()));
        let Some(path) = selected else {
            return Ok(Outcome::Cancelled);
        };
        Some(path)
    } else {
        None
    };
    // Load before saving the old document: an invalid open must leave the
    // old document and its on-disk baseline unchanged.
    let mut opened = if let Some(path) = &open_path {
        let buffer = Buffer::open_with_policy(path, HugeFilePolicy::default())
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        Some((Some(path.clone()), buffer))
    } else if matches!(request, Request::New) {
        Some((
            None,
            Buffer::from_bytes(b"").expect("empty document decodes"),
        ))
    } else {
        None
    };
    let mut saved = None;
    let is_reopening_current_file = open_path.is_some() && open_path == snapshot.path;
    let requires_save_confirmation = matches!(request, Request::Close) || is_reopening_current_file;
    if snapshot.dirty && requires_save_confirmation {
        let name = snapshot
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or("Untitled".into());
        match dialogs.confirm_save(&name) {
            SaveDecision::Cancel => return Ok(Outcome::Cancelled),
            SaveDecision::Discard => {}
            SaveDecision::Save => {
                saved = save_snapshot(&snapshot, false, dialogs)?;
                if saved.is_none() {
                    return Ok(Outcome::Cancelled);
                }
            }
        }
    }
    // Reopening the document just saved (including a symlink alias) must
    // display the saved snapshot, not the older pre-confirmation read.
    if let (Some((saved_path, saved_buffer)), Some((Some(open_path), opened_buffer))) =
        (&saved, &mut opened)
        && let (Ok(saved_target), Ok(open_target)) = (
            std::fs::canonicalize(saved_path),
            std::fs::canonicalize(open_path),
        )
        && saved_target == open_target
    {
        *opened_buffer = saved_buffer.clone();
    }
    Ok(Outcome::Finished {
        saved,
        opened,
        close: matches!(request, Request::Close),
        saved_paths: Vec::new(),
    })
}

fn save_named_files(files: &[NamedSave], dialogs: &dyn FileDialogs) -> Result<Outcome, String> {
    let mut saved_paths = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        let snapshot = Snapshot {
            buffer: file.buffer.clone(),
            path: Some(file.path.clone()),
            dirty: true,
            disk: file.disk.clone(),
        };
        match save_snapshot(&snapshot, false, dialogs) {
            Ok(Some((path, _))) => saved_paths.push(path),
            Ok(None) => {}
            Err(error) => errors.push(error),
        }
    }
    if saved_paths.is_empty() && !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(Outcome::Finished {
        saved: None,
        opened: None,
        close: false,
        saved_paths,
    })
}

pub(crate) fn recovery_store() -> Option<lux_workspace::session::RecoveryStore> {
    let dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir)
        .join("lux");
    Some(lux_workspace::session::RecoveryStore::new(dir))
}
