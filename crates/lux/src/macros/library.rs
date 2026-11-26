//! Saved `.sublime-macro` files: where they live, reading them on a worker,
//! listing them in the command palette as "Macro: <name>", and playing one.

use std::path::{Path, PathBuf};

use lux_ui::command_palette::{Command, CommandRegistry};

use super::macro_def::Macro;
use crate::app::App;

/// Palette command ids for saved macros are this prefix plus the name.
pub(crate) const RUN_PREFIX: &str = "macro.run:";

/// Longest accepted macro name, in characters.
const MAX_NAME_CHARS: usize = 64;

/// Suffix of a workspace macro's palette name when a user macro already has
/// its name.
pub(crate) const WORKSPACE_SUFFIX: &str = " (workspace)";

/// Checks a name typed into the Save Macro prompt and returns it without
/// surrounding blanks or a trailing `.sublime-macro`.
///
/// Names become file names, and a macro folder may be synced between
/// machines, so the same rules hold on every platform: letters, digits,
/// spaces, `-`, `_` and `.` only, no `.` or space at either end, at most
/// [`MAX_NAME_CHARS`] characters, and none of the Windows device names
/// (`CON`, `NUL`, `COM1`, …). That excludes path separators, drive and
/// stream colons (`C:x`, `a:b`) and `* ? " < > |`.
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
    if name.chars().count() > MAX_NAME_CHARS {
        return invalid("macro names are at most 64 characters");
    }
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.');
    if !name.chars().all(allowed) || name.starts_with('.') || name.ends_with('.') {
        return invalid(
            "macro names may use letters, digits, spaces, '-', '_' and '.' (not first or last)",
        );
    }
    if is_windows_device_name(name) {
        return invalid("that name is reserved for a device on Windows");
    }
    Ok(name)
}

/// `CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9` and `LPT0`–`LPT9` (with
/// superscript digits too), in any case, alone or before an extension.
fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let upper = stem.to_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let mut chars = upper.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let rest: Vec<char> = chars.collect();
    matches!(prefix.as_str(), "COM" | "LPT")
        && matches!(rest.as_slice(), [d] if d.is_ascii_digit() || matches!(d, '¹' | '²' | '³'))
}

/// Where saved macros are read from.
///
/// User folders come first, lowest precedence first, and end with the one
/// Save Macro writes to (`~/.lux/macros`), so a macro you save always wins
/// over a same-named one in the other user folders. The workspace's
/// `.lux/macros` never shadows a user macro: on a clash both are listed and
/// the workspace one is called "<name> (workspace)".
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct MacroDirs {
    pub(crate) user: Vec<PathBuf>,
    pub(crate) workspace: Option<PathBuf>,
}

pub(crate) fn macro_dirs(workspace: Option<&Path>) -> MacroDirs {
    MacroDirs {
        user: user_dirs(),
        workspace: workspace.map(|ws| ws.join(".lux").join("macros")),
    }
}

#[cfg(test)]
fn user_dirs() -> Vec<PathBuf> {
    // Tests never read the real home folders: only the test folder, if the
    // test set one.
    super::recording::test_dir().into_iter().collect()
}

#[cfg(not(test))]
fn user_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from) {
        dirs.push(xdg.join("lux").join("macros"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join(".config").join("lux").join("macros"));
    }
    dirs.push(super::recording::default_macros_dir());
    dirs
}

/// Reads every `.sublime-macro` under `dirs` (recursively), sorted by name.
/// A name found in a later directory replaces the same name from an earlier
/// one. Runs on a worker: it touches the disk.
pub(crate) fn read_macros(dirs: &[PathBuf]) -> Vec<Macro> {
    let mut found: Vec<Macro> = Vec::new();
    for dir in dirs {
        read_dir_into(dir, &mut found);
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Reads the user folders and the workspace folder (see [`MacroDirs`]).
/// Runs on a worker.
pub(crate) fn read_library(dirs: &MacroDirs) -> Vec<Macro> {
    let mut found = read_macros(&dirs.user);
    let workspace = dirs
        .workspace
        .as_ref()
        .map(|ws| read_macros(std::slice::from_ref(ws)))
        .unwrap_or_default();
    for mut m in workspace {
        if found.iter().any(|user| user.name == m.name) {
            m.name = m.name.map(|name| format!("{name}{WORKSPACE_SUFFIX}"));
        }
        found.retain(|existing| existing.name != m.name);
        found.push(m);
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
/// list when it finishes, unless a later load started meanwhile. `extra` is
/// searched last among the user folders (a directory a macro was just saved
/// to, if it is not one of the usual ones).
pub(crate) fn start_loading(app: &mut App, extra: Option<PathBuf>) {
    let mut dirs = macro_dirs(app.workspace_root().as_deref());
    if let Some(extra) = extra
        && !dirs.user.contains(&extra)
        && dirs.workspace.as_ref() != Some(&extra)
    {
        dirs.user.push(extra);
    }
    let generation = app.ui.macros.begin_load();
    crate::ui_io::spawn(app, None, move || {
        Some(crate::ui_io::Done::MacrosLoaded {
            generation,
            macros: read_library(&dirs),
        })
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
        for good in [
            "wrap lines",
            "a-b_c.d",
            "Ünïcode 42",
            "comma",
            "console",
            "COM10",
            "LPT",
        ] {
            assert_eq!(valid_name(good).unwrap(), good, "{good:?}");
        }
        assert!(valid_name(&"x".repeat(64)).is_ok());
        let separators = ["../x", "a/b", "a\\b", "/abs", "\\\\server\\share"];
        let windows = [
            "C:evil",
            "c:",
            "a:b",
            "x*y",
            "x?y",
            "x\"y",
            "x<y",
            "x>y",
            "x|y",
            "trailing.",
            "dots..",
        ];
        let devices = [
            "CON",
            "con",
            "Prn",
            "AUX",
            "NUL",
            "nul.txt",
            "NUL .x",
            "COM1",
            "com9",
            "COM0",
            "LPT1",
            "lpt9",
            "COM\u{b9}",
            "LPT\u{b2}",
        ];
        let other = [
            "",
            "   ",
            ".hidden",
            ".sublime-macro",
            "tab\there",
            "nl\nthere",
            "paren(s)",
            "dollar$",
        ];
        let too_long = "x".repeat(65);
        for bad in separators
            .iter()
            .chain(&windows)
            .chain(&devices)
            .chain(&other)
            .copied()
            .chain([too_long.as_str()])
        {
            assert!(valid_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_saved_name_always_lands_directly_in_the_macro_folder() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = super::super::MacroState::default();
        state.last_macro = Some(Macro::from_commands(vec![
            super::super::MacroCommand::InsertNewline,
        ]));
        for name in ["wrap", "a b", "x.y"] {
            let (path, _) = state.prepare_save(name, Some(dir.path())).unwrap();
            assert_eq!(path.parent(), Some(dir.path()), "{name}");
        }
        for name in ["C:evil", "a:b", "../x", "CON"] {
            assert!(
                state.prepare_save(name, Some(dir.path())).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn a_workspace_macro_never_shadows_a_user_macro() {
        let user = tempfile::tempdir().unwrap();
        let ws = tempfile::tempdir().unwrap();
        let write = |dir: &Path, name: &str, text: &str| {
            let m = Macro::from_commands(vec![super::super::MacroCommand::InsertText {
                text: text.into(),
            }]);
            m.save_to_file(&dir.join(format!("{name}.sublime-macro")))
                .unwrap();
        };
        write(user.path(), "wrap", "user");
        write(ws.path(), "wrap", "workspace");
        write(ws.path(), "only", "workspace only");
        let found = read_library(&MacroDirs {
            user: vec![user.path().to_path_buf()],
            workspace: Some(ws.path().to_path_buf()),
        });
        let names: Vec<_> = found.iter().filter_map(|m| m.name.as_deref()).collect();
        assert_eq!(names, ["only", "wrap", "wrap (workspace)"]);
        let text = |m: &Macro| match &m.commands[0] {
            super::super::MacroCommand::InsertText { text } => text.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(text(&found[1]), "user");
        assert_eq!(text(&found[2]), "workspace");
    }

    #[test]
    fn tests_read_no_home_folder_without_a_test_dir() {
        crate::macros::recording::set_test_dir(None);
        assert!(macro_dirs(None).user.is_empty());
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
