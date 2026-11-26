//! Project Build System runner, build variants picker, and compiler error navigation (T05, ST13).

use std::path::{Path, PathBuf};

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_term::{CommandBuilder, SourceLocation, TaskId, detect_links};

use crate::app::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildVariant {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) file_regex: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuildPickerState {
    pub(crate) variants: Vec<BuildVariant>,
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected: usize,
}

impl BuildPickerState {
    pub(crate) fn filtered(&self) -> Vec<&BuildVariant> {
        let q = self.query.trim();
        if q.is_empty() {
            return self.variants.iter().collect();
        }
        let labels: Vec<(usize, String)> = self
            .variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                (
                    index,
                    format!(
                        "{} {} {}",
                        variant.name,
                        variant.command,
                        variant.args.join(" ")
                    ),
                )
            })
            .collect();
        lux_search::fuzzy::rank_by(q, &labels, |(_, label)| label.as_str())
            .into_iter()
            .map(|(item, _)| &self.variants[item.0])
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected + 1 < total {
            self.selected += 1;
        }
    }

    #[cfg(test)]
    pub(crate) fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct BuildState {
    pub(crate) active_variant: Option<BuildVariant>,
    pub(crate) is_running: bool,
    pub(crate) task_id: Option<TaskId>,
    pub(crate) detected_errors: Vec<SourceLocation>,
    pub(crate) current_error_idx: Option<usize>,
    pub(crate) picker: Option<BuildPickerState>,
}

pub(crate) fn detect_build_variants(
    workspace_root: Option<&Path>,
    current_file: Option<&Path>,
    custom_builds: &[lux_workspace::project::ProjectBuildSystem],
) -> Vec<BuildVariant> {
    let mut variants = Vec::new();

    for cb in custom_builds {
        if !cb.cmd.is_empty() {
            variants.push(BuildVariant {
                name: cb.name.clone(),
                command: cb.cmd[0].clone(),
                args: cb.cmd[1..].to_vec(),
                file_regex: cb.file_regex.clone(),
            });
        }
    }

    let has_cargo = workspace_root.is_some_and(|r| r.join("Cargo.toml").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "Cargo.toml"));

    if has_cargo {
        variants.push(BuildVariant {
            name: "Cargo: Check".into(),
            command: "cargo".into(),
            args: vec!["check".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Cargo: Build".into(),
            command: "cargo".into(),
            args: vec!["build".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Cargo: Test".into(),
            command: "cargo".into(),
            args: vec!["test".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Cargo: Clippy".into(),
            command: "cargo".into(),
            args: vec!["clippy".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Cargo: Run".into(),
            command: "cargo".into(),
            args: vec!["run".into()],
            file_regex: None,
        });
    }

    let has_package_json = workspace_root.is_some_and(|r| r.join("package.json").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "package.json"));

    if has_package_json {
        variants.push(BuildVariant {
            name: "npm: test".into(),
            command: "npm".into(),
            args: vec!["test".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "npm: build".into(),
            command: "npm".into(),
            args: vec!["run".into(), "build".into()],
            file_regex: None,
        });
    }

    if let Some(file) = current_file {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();

        match ext {
            "js" | "mjs" | "cjs" | "ts" => {
                variants.push(BuildVariant {
                    name: "Node: Run File".into(),
                    command: "node".into(),
                    args: vec![file_str],
                    file_regex: None,
                });
            }
            "py" => {
                variants.push(BuildVariant {
                    name: "Python: Run File".into(),
                    command: "python3".into(),
                    args: vec![file_str],
                    file_regex: None,
                });
            }
            "go" => {
                variants.push(BuildVariant {
                    name: "Go: Run File".into(),
                    command: "go".into(),
                    args: vec!["run".into(), file_str],
                    file_regex: None,
                });
            }
            "sh" | "bash" => {
                variants.push(BuildVariant {
                    name: "Bash: Run File".into(),
                    command: "bash".into(),
                    args: vec![file_str],
                    file_regex: None,
                });
            }
            "rs" => {
                variants.push(BuildVariant {
                    name: "Rust: Compile File".into(),
                    command: "rustc".into(),
                    args: vec![file_str],
                    file_regex: None,
                });
            }
            _ => {}
        }
    }

    let has_pyproject = workspace_root
        .is_some_and(|r| r.join("pyproject.toml").exists() || r.join("pytest.ini").exists());
    if has_pyproject {
        variants.push(BuildVariant {
            name: "Python: Pytest".into(),
            command: "pytest".into(),
            args: vec![],
            file_regex: None,
        });
    }

    let has_go = workspace_root.is_some_and(|r| r.join("go.mod").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "go.mod"));
    if has_go {
        variants.push(BuildVariant {
            name: "Go: Build".into(),
            command: "go".into(),
            args: vec!["build".into()],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Go: Test".into(),
            command: "go".into(),
            args: vec!["test".into(), "./...".into()],
            file_regex: None,
        });
    }

    let has_makefile =
        workspace_root.is_some_and(|r| r.join("Makefile").exists() || r.join("makefile").exists());
    if has_makefile {
        variants.push(BuildVariant {
            name: "Make".into(),
            command: "make".into(),
            args: vec![],
            file_regex: None,
        });
        variants.push(BuildVariant {
            name: "Make: Clean".into(),
            command: "make".into(),
            args: vec!["clean".into()],
            file_regex: None,
        });
    }

    variants
}

fn has_ancestor_file(path: &Path, file_name: &str) -> bool {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join(file_name).exists() {
            return true;
        }
        current = dir.parent();
    }
    false
}

pub(crate) fn parse_build_errors(
    text: &str,
    variant: Option<&BuildVariant>,
) -> Vec<SourceLocation> {
    if let Some(regex_str) = variant.and_then(|v| v.file_regex.as_deref())
        && let Ok(re) = regex::Regex::new(regex_str)
    {
        let mut locs = Vec::new();
        for line in text.lines() {
            if let Some(caps) = re.captures(line)
                && let Some(p) = caps.get(1)
            {
                let path = PathBuf::from(p.as_str());
                let line_n: u32 = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(1);
                let col_n: Option<u32> = caps.get(3).and_then(|m| m.as_str().parse().ok());
                locs.push(SourceLocation {
                    path,
                    line: line_n,
                    column: col_n,
                });
            }
        }
        if !locs.is_empty() {
            return locs;
        }
    }

    detect_links(text)
}

pub(crate) fn handle_build_key(app: &mut App, ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    let total = {
        let Some(picker) = app.ui.build.picker.as_ref() else {
            return false;
        };
        picker.filtered().len()
    };
    let metrics = crate::modal::ModalMetrics::from_app(app);
    let visible = crate::overlay_list::visible_rows(
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
    );

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.build.picker = None;
            app.scheduler.invalidate_all();
            true
        }
        key if {
            let Some(picker) = app.ui.build.picker.as_mut() else {
                return false;
            };
            crate::overlay_list::nudge(&mut picker.selected, total, visible, key)
        } =>
        {
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.build.picker.as_mut() {
                picker.query.pop();
                picker.selected = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let selected_variant = {
                let Some(picker) = app.ui.build.picker.as_ref() else {
                    return false;
                };
                picker.filtered().get(picker.selected).copied().cloned()
            };
            app.ui.build.picker = None;
            if let Some(variant) = selected_variant {
                app.execute_build_variant(variant, Some(ctx));
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) if !event.repeat && !crate::field::is_shortcut_chord(app) => {
            if let Some(picker) = app.ui.build.picker.as_mut() {
                picker.query.push_str(s);
                picker.selected = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(_) => false,
        _ => true,
    }
}

impl App {
    pub(crate) fn build(&mut self, ctx: Option<&dyn PlatformContext>) -> bool {
        let variant = self.ui.build.active_variant.clone().or_else(|| {
            let root = self.workspace_root();
            let cur = self.documents.path.as_deref();
            let custom_builds = self.ui.project.custom_build_systems();
            detect_build_variants(root.as_deref(), cur, custom_builds)
                .into_iter()
                .next()
        });

        let Some(variant) = variant else {
            self.ui.announcement = Some("No build system found for this project".into());
            self.scheduler.invalidate_all();
            return false;
        };

        self.execute_build_variant(variant, ctx)
    }

    pub(crate) fn build_with(&mut self) -> bool {
        let root = self.workspace_root();
        let cur = self.documents.path.as_deref();
        let custom_builds = self.ui.project.custom_build_systems();
        let variants = detect_build_variants(root.as_deref(), cur, custom_builds);
        if variants.is_empty() {
            self.ui.announcement = Some("No build system variants found".into());
            self.scheduler.invalidate_all();
            return false;
        }

        self.ui.build.picker = Some(BuildPickerState {
            variants,
            query: crate::field::QueryField::new(),
            selected: 0,
        });
        self.scheduler.invalidate_all();
        true
    }

    pub(crate) fn cancel_build(&mut self) -> bool {
        if let Some(task_id) = self.ui.build.task_id {
            self.ui.term.cancel_task(task_id);
            self.ui.build.task_id = None;
            self.ui.build.is_running = false;
            self.ui.announcement = Some("Build cancelled".into());
            self.scheduler.invalidate_all();
            return true;
        }
        false
    }

    pub(crate) fn execute_build_variant(
        &mut self,
        variant: BuildVariant,
        _ctx: Option<&dyn PlatformContext>,
    ) -> bool {
        let root = self.workspace_root();
        let mut cmd = CommandBuilder::new(&variant.command);
        cmd.args(&variant.args);
        if let Some(r) = root.as_deref() {
            cmd.cwd(r);
        }

        let task_name = format!("Build: {}", variant.name);
        match self.ui.term.run_task(&task_name, cmd) {
            Ok(task_id) => {
                self.ui.build.active_variant = Some(variant);
                self.ui.build.task_id = Some(task_id);
                self.ui.build.is_running = true;
                self.ui.build.detected_errors.clear();
                self.ui.build.current_error_idx = None;
                self.ui.build.picker = None;
                self.ui.term.visible = true;
                self.ui.term.focused = true;
                self.ui.announcement = Some(format!("Running {}", task_name));
                self.scheduler.invalidate_all();
                true
            }
            Err(e) => {
                self.ui.announcement = Some(format!("Failed to start build: {}", e));
                self.scheduler.invalidate_all();
                false
            }
        }
    }

    pub(crate) fn sync_build_errors(&mut self) {
        if let Some(task_id) = self.ui.build.task_id
            && let Some(pty) = self.ui.term.tasks.get(task_id)
        {
            let text = pty.snapshot();
            let errors = parse_build_errors(&text, self.ui.build.active_variant.as_ref());
            if !errors.is_empty() {
                self.ui.build.detected_errors = errors;
            }
        }
    }

    pub(crate) fn next_build_error(&mut self, ctx: Option<&dyn PlatformContext>) -> bool {
        self.step_build_error(true, ctx)
    }

    pub(crate) fn prev_build_error(&mut self, ctx: Option<&dyn PlatformContext>) -> bool {
        self.step_build_error(false, ctx)
    }

    fn step_build_error(&mut self, forward: bool, ctx: Option<&dyn PlatformContext>) -> bool {
        self.sync_build_errors();

        if self.ui.build.detected_errors.is_empty() {
            return false;
        }

        let total = self.ui.build.detected_errors.len();
        let next_idx = match self.ui.build.current_error_idx {
            None => {
                if forward {
                    0
                } else {
                    total - 1
                }
            }
            Some(curr) => {
                if forward {
                    (curr + 1) % total
                } else if curr == 0 {
                    total - 1
                } else {
                    curr - 1
                }
            }
        };

        self.ui.build.current_error_idx = Some(next_idx);
        let loc = self.ui.build.detected_errors[next_idx].clone();
        self.jump_to_source_location(&loc, ctx);
        self.ui.announcement = Some(format!(
            "Build Error {} of {}: {}:{}:{}",
            next_idx + 1,
            total,
            loc.path.display(),
            loc.line,
            loc.column.unwrap_or(1)
        ));
        self.scheduler.invalidate_all();
        true
    }

    pub(crate) fn jump_to_source_location(
        &mut self,
        loc: &SourceLocation,
        ctx: Option<&dyn PlatformContext>,
    ) {
        let path = if loc.path.is_absolute() {
            loc.path.clone()
        } else if let Some(root) = self.workspace_root() {
            root.join(&loc.path)
        } else {
            loc.path.clone()
        };

        let existing_tab = self.documents.entries.iter().find(|e| {
            e.path.as_ref().is_some_and(|p| p == &path)
                || self
                    .documents
                    .tabs
                    .tabs()
                    .iter()
                    .find(|t| t.id == e.tab_id)
                    .is_some_and(|t| t.title == path.display().to_string())
        });

        if let Some(entry) = existing_tab {
            let tab_id = entry.tab_id;
            self.switch_to_tab(tab_id);
        } else if path.exists() {
            if let Ok(buffer) = lux_core::buffer::Buffer::open(&path) {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("File");
                let tab_id = self.documents.tabs.open(name);
                let entry = crate::document::DocumentEntry::new(tab_id, Some(path), buffer);
                self.documents.entries.push(entry);
                self.switch_to_tab(tab_id);
            } else if let Some(ctx) = ctx {
                self.request_document(ctx, crate::document::Request::Open(Some(path)));
            }
        }

        crate::goto::jump_to_line_and_col(self, loc.line, loc.column);
    }
}
