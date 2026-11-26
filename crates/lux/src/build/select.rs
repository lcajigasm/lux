//! Which build runs: the active buffer's Sublime base scope, every build
//! system that applies to it, and the automatic choice for `Cmd+B`.

use std::path::Path;

use lux_config::build_system::{self, BuildCommand, BuildSystem, ExpandContext, ResolvedBuild};
use lux_syntax::LanguageId;

use super::{BuildVariant, detect_build_variants};
use crate::app::App;

/// Sublime Text's base scope for a Lux language, the scope `.sublime-build`
/// `selector`s are written against. Differs from `LanguageId::scope` only
/// where Sublime's default syntaxes name the scope differently.
pub(crate) fn sublime_scope(language: LanguageId) -> &'static str {
    match language {
        LanguageId::Cpp => "source.c++",
        LanguageId::Bash => "source.shell.bash",
        LanguageId::Html => "text.html.basic",
        LanguageId::Markdown => "text.html.markdown",
        LanguageId::Scss => "source.scss",
        other => other.scope(),
    }
}

/// The active buffer's base scope: its language, else one guessed from
/// the file name.
pub(crate) fn active_scope(app: &App) -> Option<&'static str> {
    app.documents
        .language()
        .or_else(|| LanguageId::detect(app.documents.path.as_deref(), None))
        .map(sublime_scope)
}

fn expand_context(app: &App) -> ExpandContext {
    ExpandContext {
        file: app.documents.path.clone(),
        folder: app.workspace_root(),
        project: app.ui.project.active_project_path.clone(),
    }
}

/// The project file's `build_systems`, which take the same keys as a
/// `.sublime-build` file. Already parsed with the project: no I/O here.
fn project_systems(app: &App) -> Vec<BuildSystem> {
    app.ui
        .project
        .custom_build_systems()
        .iter()
        .filter_map(|entry| {
            let value = serde_json::to_value(entry).ok()?;
            BuildSystem::from_value(&entry.name, &value).ok()
        })
        .collect()
}

/// The command a `shell_cmd` runs through: the user's shell with `-c`, or
/// `cmd.exe /C` on Windows.
fn shell_argv(line: String) -> (String, Vec<String>) {
    if cfg!(windows) {
        let shell = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into());
        (shell, vec!["/C".into(), line])
    } else {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/bin/sh".into());
        (shell, vec!["-c".into(), line])
    }
}

pub(crate) fn variant_from(resolved: ResolvedBuild, selector: Option<&str>) -> BuildVariant {
    let (command, args) = match resolved.command {
        BuildCommand::Argv(mut argv) => {
            let command = if argv.is_empty() {
                String::new()
            } else {
                argv.remove(0)
            };
            (command, argv)
        }
        BuildCommand::Shell(line) => shell_argv(line),
    };
    BuildVariant {
        name: resolved.name,
        command,
        args,
        file_regex: resolved.file_regex,
        line_regex: resolved.line_regex,
        working_dir: resolved.working_dir,
        env: resolved.env,
        selector: selector.map(str::to_string),
    }
}

/// Every build that applies to the active buffer, in Build With order:
/// systems whose `selector` matches (best match first), then project and
/// `.sublime-build` systems without a selector, then auto-detected ones.
/// Systems whose selector does not match are left out, as in Sublime.
pub(crate) fn candidates(app: &App) -> Vec<BuildVariant> {
    let scope = active_scope(app);
    let context = expand_context(app);
    let mut systems = project_systems(app);
    systems.extend(app.ui.build.systems.systems.iter().cloned());

    let mut matched: Vec<(u32, &BuildSystem)> = systems
        .iter()
        .filter_map(|s| s.selector_score(scope).map(|score| (score, s)))
        .collect();
    matched.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let unscoped = systems.iter().filter(|s| s.selector.is_none());

    let mut out: Vec<BuildVariant> = matched
        .into_iter()
        .map(|(_, s)| s)
        .chain(unscoped)
        .flat_map(|system| {
            let selector = system.selector.as_deref();
            system
                .resolve(&context)
                .into_iter()
                .map(move |resolved| variant_from(resolved, selector))
        })
        .filter(|v| !v.command.is_empty())
        .collect();
    let root = app.workspace_root();
    out.extend(detect_build_variants(
        root.as_deref(),
        app.documents.path.as_deref(),
    ));
    out
}

/// Whether a variant may run for the buffer whose base scope is `scope`.
pub(crate) fn applies(variant: &BuildVariant, scope: Option<&str>) -> bool {
    match variant.selector.as_deref() {
        None => true,
        Some(selector) => scope.is_some_and(|scope| build_system::score(selector, scope).is_some()),
    }
}

/// `Cmd+B`: the variant picked in Build With while it still applies, else
/// the first candidate (the best-matching build system's own command).
/// The picked variant is re-resolved so `${file}` follows the active file.
pub(crate) fn automatic(app: &App) -> Option<BuildVariant> {
    let scope = active_scope(app);
    let mut candidates = candidates(app);
    if let Some(chosen) = app.ui.build.chosen.as_ref()
        && applies(chosen, scope)
    {
        if let Some(index) = candidates.iter().position(|v| v.name == chosen.name) {
            return Some(candidates.swap_remove(index));
        }
        return Some(chosen.clone());
    }
    candidates.into_iter().next()
}

/// `.sublime-build` files are matched by extension.
pub(crate) fn is_build_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext == build_system::SUBLIME_BUILD_EXT)
}
