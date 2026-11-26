//! `.sublime-build` files on disk: read on a worker at startup, and again
//! whenever one changes. User files live in `builds/` next to the user
//! settings file (`~/.config/lux/builds/`, created on first load), watched
//! by its own file watcher. Project files are `*.sublime-build` at the top
//! of each project folder and anywhere under `<folder>/.lux/builds/`;
//! the project's file watcher reports their changes. Nothing polls.

use std::path::PathBuf;
use std::time::Duration;

use lux_config::build_system::{self, BuildSystem};
use lux_workspace::watcher::{FileChange, FileWatcher};

use super::select::is_build_file;
use crate::app::App;
use crate::ui_io::Done;

#[derive(Default)]
pub(crate) struct Registry {
    /// Systems from the last finished load: project files first, then
    /// the user's.
    pub(crate) systems: Vec<BuildSystem>,
    /// Files that failed to read or parse in the last load.
    pub(crate) errors: Vec<String>,
    /// Bumped by every reload so an older load landing late is dropped.
    generation: u64,
    /// The generation the current `systems` came from.
    loaded_generation: u64,
    /// The session's folder epoch the last reload was asked for. `None`
    /// until the startup load; folder changes reload after it.
    loaded_epoch: Option<u64>,
    watcher: Option<FileWatcher>,
    watching: Option<PathBuf>,
}

#[cfg(test)]
impl Registry {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn loaded_generation(&self) -> u64 {
        self.loaded_generation
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("systems", &self.systems.len())
            .field("generation", &self.generation)
            .field("watching", &self.watching)
            .finish_non_exhaustive()
    }
}

/// A finished load, applied on the UI thread.
pub(crate) struct Loaded {
    generation: u64,
    systems: Vec<BuildSystem>,
    errors: Vec<String>,
    watcher: Option<(PathBuf, FileWatcher)>,
}

/// The user's build folder: `builds/` beside the user settings file.
pub(crate) fn user_dir(app: &App) -> Option<PathBuf> {
    app.ui
        .settings
        .settings_path
        .parent()
        .map(|dir| dir.join("builds"))
}

fn project_dirs(app: &App) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = app
        .ui
        .project
        .workspace
        .as_ref()
        .map(|ws| ws.roots().iter().map(|r| r.path.clone()).collect())
        .unwrap_or_default();
    if let Some(root) = app.workspace_root()
        && !dirs.contains(&root)
    {
        dirs.push(root);
    }
    dirs
}

/// Starts a load on a worker. The result replaces the registry when it
/// lands through `ui_io::poll`.
pub(crate) fn reload(app: &mut App) {
    let registry = &mut app.ui.build.systems;
    registry.generation += 1;
    registry.loaded_epoch = Some(app.ui.session.folder_epoch());
    let generation = registry.generation;
    let user = user_dir(app);
    let watch = user
        .clone()
        .filter(|dir| app.ui.build.systems.watching.as_ref() != Some(dir));
    let projects = project_dirs(app);
    let waker = app.ui.session.waker();
    crate::ui_io::spawn(app, None, move || {
        let mut systems = Vec::new();
        let mut errors = Vec::new();
        let mut take = |(found, failed): (Vec<BuildSystem>, Vec<build_system::BuildParseError>)| {
            systems.extend(found);
            errors.extend(failed.into_iter().map(|e| e.0));
        };
        for dir in &projects {
            take(build_system::load_dir_shallow(dir));
            take(build_system::load_dir(&dir.join(".lux").join("builds")));
        }
        if let Some(dir) = &user {
            take(build_system::load_dir(dir));
        }
        let watcher = watch.and_then(|dir| {
            std::fs::create_dir_all(&dir).ok()?;
            let mut watcher = FileWatcher::new(Duration::from_millis(150)).ok()?;
            if let Some(waker) = waker {
                watcher.set_waker(waker);
            }
            watcher.watch(&dir).ok()?;
            Some((dir, watcher))
        });
        Some(Done::BuildSystems(Box::new(Loaded {
            generation,
            systems,
            errors,
            watcher,
        })))
    });
}

pub(crate) fn apply_loaded(app: &mut App, loaded: Loaded) {
    let registry = &mut app.ui.build.systems;
    if let Some((dir, watcher)) = loaded.watcher {
        registry.watcher = Some(watcher);
        registry.watching = Some(dir);
    }
    if loaded.generation != registry.generation {
        return;
    }
    registry.loaded_generation = loaded.generation;
    registry.systems = loaded.systems;
    registry.errors = loaded.errors;
    if let Some(error) = registry.errors.first() {
        app.ui.announcement = Some(format!("Build system not loaded: {error}"));
    }
}

/// Drains the user folder's watcher, and reloads when a build file there
/// changed or the open folders changed since the last load. Runs from
/// the event loop's file-watcher drain; both triggers are events.
pub(crate) fn poll(app: &mut App) {
    let Some(epoch) = app.ui.build.systems.loaded_epoch else {
        return;
    };
    let folders_changed = epoch != app.ui.session.folder_epoch();
    let files_changed = app
        .ui
        .build
        .systems
        .watcher
        .as_mut()
        .map(|w| w.try_recv())
        // Everything under the user folder is build files and folders.
        .is_some_and(|batch| !batch.changes.is_empty());
    if folders_changed || files_changed {
        reload(app);
    }
}

/// A batch from the project's watcher. Reloads when it touched a build
/// file, once the startup load has run.
pub(crate) fn note_disk_changes(app: &mut App, changes: &[FileChange]) {
    if app.ui.build.systems.loaded_epoch.is_some() && touches_build_files(changes) {
        reload(app);
    }
}

/// A build file, or a `builds` folder (created, or removed with its
/// files: a removed folder reports `is_dir: false`).
fn touches_build_files(changes: &[FileChange]) -> bool {
    changes.iter().any(|change| {
        is_build_file(&change.path) || change.path.file_name().is_some_and(|n| n == "builds")
    })
}
