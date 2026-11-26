//! Saved `.sublime-macro` files: where they live, reading them on a worker,
//! listing them in the command palette as "Macro: <name>", and playing one.

use std::path::{Path, PathBuf};

use lux_ui::command_palette::{Command, CommandRegistry};

use super::macro_def::Macro;
use crate::app::App;

/// Palette command ids for saved macros are this prefix plus the name.
pub(crate) const RUN_PREFIX: &str = "macro.run:";

/// Checks a name typed into the Save Macro prompt and returns it without
/// surrounding blanks or a trailing `.sublime-macro`. Names become file
/// names, so path separators and leading dots are refused.
pub(crate) fn valid_name(name: &str) -> std::io::Result<&str> {
    let name = name.trim();
    let name = name.strip_suffix(".sublime-macro").unwrap_or(name).trim();
    let invalid = |why: &str| {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            why.to_string(),
        ))
    };
    if name.is_empty() {
        return invalid("a macro needs a name");
    }
    if name.starts_with('.')
        || name
            .chars()
            .any(|c| matches!(c, '/' | '\\') || c.is_control())
    {
        return invalid(
            "macro names cannot start with '.' or contain '/', '\\' or control characters",
        );
    }
    Ok(name)
}

/// Directories searched for saved macros, lowest precedence first: a name
/// found in a later directory replaces the same name from an earlier one.
pub(crate) fn macro_dirs(workspace: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(test)]
    if let Some(dir) = super::recording::test_dir() {
        dirs.push(dir);
        dirs.extend(workspace.map(|ws| ws.join(".lux").join("macros")));
        return dirs;
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        dirs.push(xdg.join("lux").join("macros"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".config").join("lux").join("macros"));
    }
    dirs.push(super::recording::default_macros_dir());
    if let Some(ws) = workspace {
        dirs.push(ws.join(".lux").join("macros"));
    }
    dirs
}

/// Reads every `.sublime-macro` under `dirs` (recursively), sorted by name.
/// Runs on a worker: it touches the disk.
pub(crate) fn read_macros(dirs: &[PathBuf]) -> Vec<Macro> {
    let mut found: Vec<Macro> = Vec::new();
    for dir in dirs {
        read_dir_into(dir, &mut found);
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

fn read_dir_into(dir: &Path, found: &mut Vec<Macro>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            read_dir_into(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some("sublime-macro")
            && let Ok(m) = Macro::load_from_file(&path)
        {
            found.retain(|existing| existing.name != m.name);
            found.push(m);
        }
    }
}

/// Reads the saved macros on a worker; `Done::MacrosLoaded` replaces the
/// list when it finishes. `extra` is searched last (a directory a macro was
/// just saved to, if it is not one of the usual ones).
pub(crate) fn start_loading(app: &mut App, extra: Option<PathBuf>) {
    let mut dirs = macro_dirs(app.workspace_root().as_deref());
    if let Some(extra) = extra
        && !dirs.contains(&extra)
    {
        dirs.push(extra);
    }
    crate::ui_io::spawn(app, None, move || {
        Some(crate::ui_io::Done::MacrosLoaded(read_macros(&dirs)))
    });
}

/// Adds one "Macro: <name>" palette command per loaded macro.
pub(crate) fn register_palette_entries(registry: &mut CommandRegistry, macros: &[Macro]) {
    for name in macros.iter().filter_map(|m| m.name.as_deref()) {
        let _ = registry.register(Command::new(
            format!("{RUN_PREFIX}{name}"),
            format!("Macro: {name}"),
            "Macros",
        ));
    }
}

/// Plays the saved macro called `name` through the regular playback, on
/// every caret. The recorded macro (`Cmd+Shift+Q`) is left as it was.
pub(crate) fn play_saved(app: &mut App, name: &str) {
    let Some(m) = app.ui.macros.saved(name).cloned() else {
        app.documents.message = Some(format!("Macro '{name}' is no longer loaded"));
        return;
    };
    super::playback::execute_macro(app, &m);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_would_escape_the_macro_folder_are_refused() {
        assert_eq!(valid_name("  wrap  ").unwrap(), "wrap");
        assert_eq!(valid_name("wrap.sublime-macro").unwrap(), "wrap");
        for bad in [
            "",
            "   ",
            "../x",
            "a/b",
            "a\\b",
            ".hidden",
            ".sublime-macro",
        ] {
            assert!(valid_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn later_directories_win_and_the_list_is_sorted() {
        let low = tempfile::tempdir().unwrap();
        let high = tempfile::tempdir().unwrap();
        let nested = low.path().join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        let write = |dir: &Path, name: &str, text: &str| {
            let m = Macro::from_commands(vec![super::super::MacroCommand::InsertText {
                text: text.into(),
            }]);
            m.save_to_file(&dir.join(format!("{name}.sublime-macro")))
                .unwrap();
        };
        write(low.path(), "zeta", "low");
        write(&nested, "alpha", "nested");
        write(high.path(), "zeta", "high");
        std::fs::write(low.path().join("notes.txt"), "ignored").unwrap();

        let found = read_macros(&[low.path().to_path_buf(), high.path().to_path_buf()]);
        let names: Vec<_> = found.iter().filter_map(|m| m.name.as_deref()).collect();
        assert_eq!(names, ["alpha", "zeta"]);
        assert_eq!(
            found[1].commands,
            [super::super::MacroCommand::InsertText {
                text: "high".into()
            }]
        );
    }
}
