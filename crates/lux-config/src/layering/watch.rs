//! Hot reload: watches each file-backed layer's path with `notify` (same
//! 8.2.0 pin as `lux-workspace`, see `docs/DEPENDENCIES.md`) and reloads +
//! remerges a [`LayeredConfig`] the moment its file changes on disk — no
//! polling, per golden rule #5. Settings files change one at a time and
//! rarely, unlike the bursts `lux-workspace::watcher` debounces across a
//! whole project tree, so this reacts to every raw `notify` event directly
//! instead of running a debouncer.
//!
//! Each layer's *parent directory* is what actually gets watched, not the
//! file itself: `notify` requires a watch target to exist, but a settings
//! file legitimately might not (nothing requires a user to have created
//! `~/.config/lux/settings.jsonc`) — watching the directory lets a file
//! that's created later start contributing without `ConfigWatcher` needing
//! to be re-created. The directory itself is still assumed to exist;
//! creating config directories is a caller/init concern, not this module's.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};

use super::{Layer, LayerError, LayeredConfig};

/// The outcome of one hot-reload attempt, delivered through
/// [`ConfigWatcher::recv`]/[`ConfigWatcher::try_recv`].
#[derive(Debug)]
pub enum ReloadEvent {
    /// `layer` reloaded successfully; the shared [`LayeredConfig`]'s merged
    /// view already reflects it by the time this arrives.
    Reloaded(Layer),
    /// `layer`'s file changed but failed to parse. The config keeps its
    /// last good value for that layer (see
    /// [`LayeredConfig::reload_layer`]) — this only tells a caller there's
    /// a problem worth surfacing.
    Failed(Layer, LayerError),
}

/// Watches every currently-set file-backed layer of a shared
/// [`LayeredConfig`] and reloads it in the background the moment its file
/// changes, with no polling anywhere in the path.
pub struct ConfigWatcher {
    config: Arc<Mutex<LayeredConfig>>,
    // Kept alive for as long as the watcher should keep running — dropping
    // it tears down the background OS watch.
    _watcher: RecommendedWatcher,
    events: Receiver<ReloadEvent>,
}

impl ConfigWatcher {
    /// Starts watching `layer -> path` pairs for `config`, loading each
    /// path synchronously first so the caller gets an already-current
    /// config back instead of racing the first filesystem event.
    pub fn new(
        config: Arc<Mutex<LayeredConfig>>,
        paths: impl IntoIterator<Item = (Layer, PathBuf)>,
    ) -> notify::Result<Self> {
        let paths: Vec<(Layer, PathBuf)> = paths.into_iter().collect();
        let by_path: Vec<(PathBuf, Layer)> = paths
            .iter()
            .map(|(layer, path)| (normalize(path), *layer))
            .collect();

        let (tx, rx) = mpsc::channel();
        let watch_config = Arc::clone(&config);
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else { return };
            if !event.kind.is_create() && !event.kind.is_modify() && !event.kind.is_remove() {
                return;
            }
            for changed in &event.paths {
                let changed = normalize(changed);
                let Some(&(_, layer)) = by_path.iter().find(|(path, _)| *path == changed) else {
                    continue;
                };
                let outcome = watch_config.lock().unwrap().reload_layer(layer);
                let reload_event = match outcome {
                    Ok(()) => ReloadEvent::Reloaded(layer),
                    Err(err) => ReloadEvent::Failed(layer, err),
                };
                // A send error means `ConfigWatcher` (and its receiver) was
                // already dropped — nothing left to notify.
                let _ = tx.send(reload_event);
            }
        })?;

        let mut watched_dirs = HashSet::new();
        for (_, path) in &paths {
            if let Some(parent) = path.parent()
                && watched_dirs.insert(parent.to_path_buf())
            {
                watcher.watch(parent, RecursiveMode::NonRecursive)?;
            }
        }

        for (layer, path) in &paths {
            // `set_layer_path` (not `reload_layer`) — it's also what
            // registers `path` on `config` in the first place, which the
            // background callback above relies on: `reload_layer` alone is
            // a no-op for a layer `config` has never heard of.
            let _ = config.lock().unwrap().set_layer_path(*layer, path.clone());
        }

        Ok(ConfigWatcher {
            config,
            _watcher: watcher,
            events: rx,
        })
    }

    /// The shared config this watcher keeps reloaded.
    pub fn config(&self) -> &Arc<Mutex<LayeredConfig>> {
        &self.config
    }

    /// Blocks up to `timeout` for the next reload outcome. This bounds how
    /// long the *caller* waits on the channel — the wait itself is a
    /// blocking channel read fed by a background thread parked on the OS's
    /// own file-change notification, not a re-check timer.
    pub fn recv(&self, timeout: Duration) -> Option<ReloadEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Non-blocking: the next already-arrived reload outcome, if any.
    pub fn try_recv(&self) -> Option<ReloadEvent> {
        self.events.try_recv().ok()
    }
}

/// Resolves symlinks so paths compare consistently between what callers
/// pass in and what the OS backend reports (macOS's FSEvents in particular
/// always reports paths through resolved symlinks, e.g. `/tmp` ->
/// `/private/tmp`). Best-effort: a path that doesn't exist yet (the whole
/// reason directories are watched instead of files, see module docs) falls
/// back to the original, unresolved path.
fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Instant;

    const RECV_TIMEOUT: Duration = Duration::from_secs(5);

    /// Replaces an *existing* watched file's contents in a single atomic
    /// step, the way a real editor's save does. `fs::write` is not atomic:
    /// its `open(O_CREAT|O_TRUNC)` empties the file before the new bytes are
    /// written, so on Linux inotify can deliver a `Modify` for the
    /// momentarily-empty file — which parses as a successful, empty `Null`
    /// document (JSONC's empty-input case), not a parse error — ahead of the
    /// `Modify` carrying the real content. Because this module deliberately
    /// reacts to every raw event (see the module docs), that intermediate
    /// state is observable and, under parallel test load, races the content
    /// write. Writing to a sibling temp file and renaming it over the target
    /// collapses the change into one event with no empty window, which is
    /// what these "edit an existing file" tests actually mean to exercise.
    /// (First-*creation* tests keep using `fs::write` on purpose — there is
    /// no prior content to truncate, and the create/modify split is part of
    /// what they assert.)
    fn atomic_replace(path: &Path, contents: &str) {
        let tmp = path.with_extension("jsonc.tmp");
        fs::write(&tmp, contents).unwrap();
        fs::rename(&tmp, path).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-config-watch-test-{label}-{:?}-{}",
            thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::canonicalize(&dir).unwrap()
    }

    fn wait_for(watcher: &ConfigWatcher, mut matches: impl FnMut(&ReloadEvent) -> bool) {
        let deadline = Instant::now() + RECV_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "timed out waiting for a reload event");
            if let Some(event) = watcher.recv(remaining)
                && matches(&event)
            {
                return;
            }
        }
    }

    #[test]
    fn starts_already_reflecting_the_files_current_contents() {
        let dir = temp_dir("initial");
        fs::write(dir.join("user.jsonc"), r#"{ "value": "user" }"#).unwrap();
        let config = Arc::new(Mutex::new(
            LayeredConfig::new(r#"{ "value": "default" }"#).unwrap(),
        ));

        let _watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, dir.join("user.jsonc"))])
                .unwrap();

        assert_eq!(config.lock().unwrap().resolved()["value"], "user");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn editing_a_watched_file_hot_reloads_without_restart() {
        let dir = temp_dir("edit");
        let path = dir.join("user.jsonc");
        fs::write(&path, r#"{ "value": "before" }"#).unwrap();
        let config = Arc::new(Mutex::new(
            LayeredConfig::new(r#"{ "value": "default" }"#).unwrap(),
        ));
        let watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, path.clone())]).unwrap();
        assert_eq!(config.lock().unwrap().resolved()["value"], "before");

        atomic_replace(&path, r#"{ "value": "after" }"#);

        wait_for(&watcher, |event| {
            matches!(event, ReloadEvent::Reloaded(Layer::User))
        });
        assert_eq!(config.lock().unwrap().resolved()["value"], "after");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn creating_a_previously_missing_file_hot_reloads() {
        let dir = temp_dir("create");
        let path = dir.join("user.jsonc");
        let config = Arc::new(Mutex::new(
            LayeredConfig::new(r#"{ "value": "default" }"#).unwrap(),
        ));
        let watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, path.clone())]).unwrap();
        assert_eq!(config.lock().unwrap().resolved()["value"], "default");

        fs::write(&path, r#"{ "value": "created" }"#).unwrap();

        // On Linux, `fs::write`'s create (`open(O_CREAT|O_TRUNC)`) and its
        // content write are two separate raw inotify events — `Create` for
        // the just-opened, still-empty file, then `Modify` once the write
        // actually lands — and this module deliberately reacts to every raw
        // event rather than debouncing (see the module doc comment).
        // Reloading the still-empty file parses as a real, successful
        // `Null` document (JSONC's empty-input case), not a parse error, so
        // it surfaces as a genuine but premature `Reloaded(Layer::User)`
        // before the one carrying the real content. Keep waiting until the
        // config actually reflects it, the same way any real consumer of
        // this event stream has to.
        wait_for(&watcher, |event| {
            matches!(event, ReloadEvent::Reloaded(Layer::User))
                && config.lock().unwrap().resolved()["value"] == "created"
        });
        assert_eq!(config.lock().unwrap().resolved()["value"], "created");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_malformed_edit_reports_failed_and_keeps_the_last_good_value() {
        let dir = temp_dir("malformed");
        let path = dir.join("user.jsonc");
        fs::write(&path, r#"{ "value": "good" }"#).unwrap();
        let config = Arc::new(Mutex::new(
            LayeredConfig::new(r#"{ "value": "default" }"#).unwrap(),
        ));
        let watcher =
            ConfigWatcher::new(Arc::clone(&config), [(Layer::User, path.clone())]).unwrap();
        assert_eq!(config.lock().unwrap().resolved()["value"], "good");

        atomic_replace(&path, "{ not valid");

        wait_for(&watcher, |event| {
            matches!(
                event,
                ReloadEvent::Failed(Layer::User, LayerError::Parse(_))
            )
        });
        assert_eq!(config.lock().unwrap().resolved()["value"], "good");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn two_layers_in_different_directories_reload_independently() {
        let user_dir = temp_dir("multi-user");
        let project_dir = temp_dir("multi-project");
        let user_path = user_dir.join("user.jsonc");
        let project_path = project_dir.join("project.jsonc");
        fs::write(&user_path, r#"{ "a": "user" }"#).unwrap();
        fs::write(&project_path, r#"{ "b": "project" }"#).unwrap();
        let config = Arc::new(Mutex::new(LayeredConfig::new(r#"{}"#).unwrap()));
        let watcher = ConfigWatcher::new(
            Arc::clone(&config),
            [
                (Layer::User, user_path.clone()),
                (Layer::Project, project_path.clone()),
            ],
        )
        .unwrap();
        assert_eq!(config.lock().unwrap().resolved()["a"], "user");
        assert_eq!(config.lock().unwrap().resolved()["b"], "project");

        atomic_replace(&project_path, r#"{ "b": "project-updated" }"#);
        wait_for(&watcher, |event| {
            matches!(event, ReloadEvent::Reloaded(Layer::Project))
        });
        assert_eq!(config.lock().unwrap().resolved()["a"], "user");
        assert_eq!(config.lock().unwrap().resolved()["b"], "project-updated");

        fs::remove_dir_all(&user_dir).ok();
        fs::remove_dir_all(&project_dir).ok();
    }
}
