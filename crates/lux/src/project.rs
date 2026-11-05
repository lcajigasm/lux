//! Project management: `.sublime-project` and `.lux-project` integration,
//! recent project tracking, and Quick Switch Project modal (ST14).

use std::fs;
use std::path::{Path, PathBuf};

use lux_platform::event::KeyEvent;
use lux_platform::{FileDialogOptions, Key, NamedKey, PlatformContext};
use lux_workspace::project::{ProjectBuildSystem, Workspace, find_sublime_project};

use crate::app::App;

const MAX_RECENT_PROJECTS: usize = 20;

/// Persisted list of recently opened project files.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct RecentProjects {
    pub(crate) paths: Vec<PathBuf>,
}

impl RecentProjects {
    pub(crate) fn file_path() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(std::env::temp_dir)
            .join("lux")
            .join("recent_projects.json")
    }

    pub(crate) fn load() -> Self {
        let path = Self::file_path();
        if path.exists()
            && let Ok(text) = fs::read_to_string(&path)
            && let Ok(recents) = serde_json::from_str::<Self>(&text)
        {
            return recents;
        }
        Self::default()
    }

    pub(crate) fn save(&self) {
        let path = Self::file_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, text);
        }
    }

    pub(crate) fn record(&mut self, path: PathBuf) {
        self.paths.retain(|p| p != &path);
        self.paths.insert(0, path);
        if self.paths.len() > MAX_RECENT_PROJECTS {
            self.paths.truncate(MAX_RECENT_PROJECTS);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remove(&mut self, path: &Path) {
        self.paths.retain(|p| p != path);
    }
}

/// Interactive state for the floating Quick Switch Project modal picker (`Cmd+Ctrl+P`).
#[derive(Debug, Clone)]
pub(crate) struct ProjectSwitchState {
    pub(crate) projects: Vec<PathBuf>,
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected: usize,
}

impl ProjectSwitchState {
    pub(crate) fn filtered(&self) -> Vec<&PathBuf> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.projects.iter().collect()
        } else {
            self.projects
                .iter()
                .filter(|p| {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let path_str = p.to_string_lossy();
                    name.to_lowercase().contains(&q) || path_str.to_lowercase().contains(&q)
                })
                .collect()
        }
    }

    pub(crate) fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected + 1 < total {
            self.selected += 1;
        }
    }

    pub(crate) fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

/// State of current project workspace and modal switch overlay.
#[derive(Debug, Default)]
pub(crate) struct ProjectState {
    pub(crate) workspace: Option<Workspace>,
    pub(crate) active_project_path: Option<PathBuf>,
    pub(crate) picker: Option<ProjectSwitchState>,
}

impl ProjectState {
    pub(crate) fn custom_build_systems(&self) -> &[ProjectBuildSystem] {
        self.workspace
            .as_ref()
            .map(|ws| ws.build_systems())
            .unwrap_or(&[])
    }
}

pub(crate) fn handle_project_switch_key(
    app: &mut App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let total = {
        let Some(picker) = app.ui.project.picker.as_ref() else {
            return false;
        };
        picker.filtered().len()
    };

    match &event.key {
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(picker) = app.ui.project.picker.as_mut() {
                picker.select_next(total);
                app.scheduler.invalidate_all();
            }
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(picker) = app.ui.project.picker.as_mut() {
                picker.select_prev();
                app.scheduler.invalidate_all();
            }
            true
        }
        Key::Named(NamedKey::Escape) => {
            app.ui.project.picker = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let selected_path = {
                let Some(picker) = app.ui.project.picker.as_ref() else {
                    return false;
                };
                picker.filtered().get(picker.selected).copied().cloned()
            };
            app.ui.project.picker = None;
            if let Some(path) = selected_path {
                app.open_project_file(&path, Some(ctx));
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.project.picker.as_mut() {
                picker.query.pop();
                picker.selected = 0;
                app.scheduler.invalidate_all();
            }
            true
        }
        Key::Character(s) => {
            let text = event.text.as_deref().unwrap_or(s.as_str());
            if !text.is_empty() && !text.chars().any(char::is_control) {
                if let Some(picker) = app.ui.project.picker.as_mut() {
                    picker.query.push_str(text);
                    picker.selected = 0;
                    app.scheduler.invalidate_all();
                }
                return true;
            }
            false
        }
        _ => false,
    }
}

impl App {
    /// Loads a `.sublime-project` or `.lux-project` file into the editor.
    pub(crate) fn open_project_file(
        &mut self,
        path: &Path,
        _ctx: Option<&dyn PlatformContext>,
    ) -> bool {
        let Ok(workspace) = Workspace::open_project_file(path) else {
            self.ui.announcement = Some(format!("Failed to open project: {}", path.display()));
            self.scheduler.invalidate_all();
            return false;
        };

        let mut recents = RecentProjects::load();
        recents.record(path.to_path_buf());
        recents.save();

        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

        if let Some(first_root) = workspace.roots().first() {
            self.ui.sidebar = crate::sidebar::SidebarState::open_folder(first_root.path.clone());
            self.ui.session.init_watcher(&first_root.path);
        } else {
            self.ui.sidebar = crate::sidebar::SidebarState::open_folder(base_dir.to_path_buf());
            self.ui.session.init_watcher(base_dir);
        }

        // Apply settings overrides
        for (k, v) in workspace.settings() {
            if k == "tab_size"
                && let Some(n) = v.as_u64()
            {
                // `tab_size` is the space width. It does not convert a
                // tab-indented file into spaces.
                let width = n.clamp(1, 16) as u8;
                if self.buffer.indent_from_content() {
                    if self.buffer.indent().style == lux_core::buffer::IndentStyle::Spaces {
                        self.buffer.set_indent_width(width);
                    }
                } else if !self.buffer.indent_is_explicit() {
                    self.buffer.set_indent(lux_core::buffer::Indent {
                        style: lux_core::buffer::IndentStyle::Spaces,
                        width,
                    });
                }
            }
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project");
        self.ui.announcement = Some(format!("Opened project: {}", name));
        self.ui.project.active_project_path = Some(path.to_path_buf());
        self.ui.project.workspace = Some(workspace);
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
        true
    }

    /// Invokes native file dialog to choose a project file to open.
    pub(crate) fn open_project_dialog(&mut self, ctx: &dyn PlatformContext) {
        let opts = FileDialogOptions {
            title: Some("Open Project".into()),
            starting_directory: self.workspace_root(),
            filters: vec![(
                "Project Files".to_string(),
                vec!["sublime-project".into(), "lux-project".into()],
            )],
        };
        if let Some(path) = ctx.file_dialogs().pick_file(&opts) {
            self.open_project_file(&path, Some(ctx));
        }
    }

    /// Invokes native file dialog to save current workspace as a `.sublime-project`.
    pub(crate) fn save_project_as_dialog(&mut self, ctx: &dyn PlatformContext) {
        let opts = FileDialogOptions {
            title: Some("Save Project As".into()),
            starting_directory: self.workspace_root(),
            filters: vec![(
                "Sublime Project".to_string(),
                vec!["sublime-project".into()],
            )],
        };
        if let Some(path) = ctx.file_dialogs().save_file(&opts) {
            self.save_project_as(&path);
        }
    }

    /// Saves the current workspace configuration to the given path.
    pub(crate) fn save_project_as(&mut self, path: &Path) -> bool {
        let root = self.workspace_root().unwrap_or_else(|| PathBuf::from("."));
        let project_file = if let Some(ws) = &self.ui.project.workspace {
            ws.to_project_file()
        } else {
            let mut ws = Workspace::new();
            ws.add_root(root, None);
            ws.to_project_file()
        };

        if project_file.save(path).is_ok() {
            self.ui.project.active_project_path = Some(path.to_path_buf());
            let mut recents = RecentProjects::load();
            recents.record(path.to_path_buf());
            recents.save();
            self.ui.announcement = Some(format!("Project saved to {}", path.display()));
            self.scheduler.invalidate_all();
            true
        } else {
            self.ui.announcement = Some(format!("Failed to save project to {}", path.display()));
            self.scheduler.invalidate_all();
            false
        }
    }

    /// Opens the Quick Switch Project modal picker (`Cmd+Ctrl+P`).
    pub(crate) fn quick_switch_project(&mut self) -> bool {
        let recents = RecentProjects::load();
        let mut projects = recents.paths;
        if projects.is_empty() {
            if let Some(active) = &self.ui.project.active_project_path {
                projects.push(active.clone());
            } else if let Some(root) = self.workspace_root()
                && let Some(sub) = find_sublime_project(&root)
            {
                projects.push(sub);
            }
        }
        if projects.is_empty() {
            self.ui.announcement = Some("No recent projects found".into());
            self.scheduler.invalidate_all();
            return false;
        }

        self.ui.project.picker = Some(ProjectSwitchState {
            projects,
            query: crate::field::QueryField::new(),
            selected: 0,
        });
        self.scheduler.invalidate_all();
        true
    }

    /// Closes the current project workspace.
    pub(crate) fn close_project(&mut self) {
        self.ui.project.active_project_path = None;
        self.ui.project.workspace = None;
        self.ui.sidebar = None;
        self.ui.announcement = Some("Project closed".into());
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }
}
