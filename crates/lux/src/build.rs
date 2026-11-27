//! Project Build System runner, build variants picker, and compiler error navigation (T05, ST13).
//! Build systems come from `.sublime-build` files (`systems`), project
//! `build_systems` and auto-detection (`detect`); `select` resolves them
//! for the active buffer.

pub(crate) mod detect;
pub(crate) mod request;
pub(crate) mod select;
pub(crate) mod systems;

#[cfg(test)]
pub(crate) use detect::detect_build_variants;

use std::path::PathBuf;

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_term::{CommandBuilder, SourceLocation, TaskId, detect_links};

use crate::app::App;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BuildVariant {
    pub(crate) name: String,
    pub(crate) command: String,
    pub(crate) args: Vec<String>,
    pub(crate) file_regex: Option<String>,
    pub(crate) line_regex: Option<String>,
    /// Where the command runs and relative error paths resolve; `None`
    /// is the workspace root.
    pub(crate) working_dir: Option<PathBuf>,
    pub(crate) env: Vec<(String, String)>,
    /// The `selector` of the build system this came from, if any.
    pub(crate) selector: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BuildPickerState {
    pub(crate) variants: Vec<BuildVariant>,
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected: usize,
}

impl BuildPickerState {
    pub(crate) fn filtered(&self) -> Vec<&BuildVariant> {
        self.ranked()
            .into_iter()
            .map(|(variant, _)| variant)
            .collect()
    }

    /// Matching variants, best first. The query ranks against
    /// `name command args`; the hits are the part of that match that falls
    /// in the name, the title the list shows.
    pub(crate) fn ranked(&self) -> Vec<(&BuildVariant, crate::overlay_list::Hits)> {
        let q = self.query.trim();
        if q.is_empty() {
            return self.variants.iter().map(|v| (v, Vec::new())).collect();
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
            .map(|ranked| {
                let (index, label) = ranked.item;
                let variant = &self.variants[*index];
                let name = 0..variant.name.len();
                let hits = lux_search::fuzzy::char_spans_in(label, &ranked.indices, name);
                (variant, hits)
            })
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
    /// The variant picked in Build With; `Cmd+B` reruns it while its
    /// selector still matches the active buffer. `None` is automatic.
    pub(crate) chosen: Option<BuildVariant>,
    pub(crate) systems: systems::Registry,
    /// Bumped by every `Cmd+B` / Build With; a marker probe landing for
    /// an older one is dropped.
    pub(crate) detect_generation: u64,
}

pub(crate) fn parse_build_errors(
    text: &str,
    variant: Option<&BuildVariant>,
) -> Vec<SourceLocation> {
    if let Some(regex_str) = variant.and_then(|v| v.file_regex.as_deref())
        && let Ok(re) = regex::Regex::new(regex_str)
    {
        // Sublime's `line_regex`: a line holding only a line (and column)
        // belongs to the file the last `file_regex` match named.
        let line_re = variant
            .and_then(|v| v.line_regex.as_deref())
            .and_then(|r| regex::Regex::new(r).ok());
        let mut last_file: Option<PathBuf> = None;
        let mut locs = Vec::new();
        for line in text.lines() {
            if let Some(caps) = re.captures(line)
                && let Some(p) = caps.get(1)
            {
                let path = PathBuf::from(p.as_str());
                last_file = Some(path.clone());
                let line_n: Option<u32> = caps.get(2).and_then(|m| m.as_str().parse().ok());
                if line_n.is_none() && line_re.is_some() {
                    continue;
                }
                let col_n: Option<u32> = caps.get(3).and_then(|m| m.as_str().parse().ok());
                locs.push(SourceLocation {
                    path,
                    line: line_n.unwrap_or(1),
                    column: col_n,
                });
            } else if let (Some(line_re), Some(path)) = (line_re.as_ref(), last_file.as_ref())
                && let Some(caps) = line_re.captures(line)
                && let Some(line_n) = caps.get(1).and_then(|m| m.as_str().parse().ok())
            {
                locs.push(SourceLocation {
                    path: path.clone(),
                    line: line_n,
                    column: caps.get(2).and_then(|m| m.as_str().parse().ok()),
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
                app.ui.build.chosen = Some(variant.clone());
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
    /// `Cmd+B`. The build starts once a worker has probed the marker
    /// files (see `request`).
    pub(crate) fn build(&mut self, ctx: Option<&dyn PlatformContext>) -> bool {
        request::start(self, ctx, request::Request::Build);
        true
    }

    /// Build With: the picker opens once the marker probe lands.
    pub(crate) fn build_with(&mut self, ctx: Option<&dyn PlatformContext>) -> bool {
        request::start(self, ctx, request::Request::BuildWith);
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
        let cwd = variant
            .working_dir
            .clone()
            .or_else(|| self.workspace_root());
        let mut cmd = CommandBuilder::new(&variant.command);
        cmd.args(&variant.args);
        if let Some(dir) = cwd.as_deref() {
            cmd.cwd(dir);
        }
        for (key, value) in &variant.env {
            cmd.env(key, value);
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
        let base = self
            .ui
            .build
            .active_variant
            .as_ref()
            .and_then(|v| v.working_dir.clone())
            .or_else(|| self.workspace_root());
        let path = if loc.path.is_absolute() {
            loc.path.clone()
        } else if let Some(root) = base {
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
            crate::goto::jump_to_line_and_col(self, loc.line, loc.column);
        } else {
            // The document worker reads the file; the cursor moves when it lands.
            crate::document::open_queue::open_at(self, ctx, path, Some(loc.line), loc.column);
        }
    }
}

#[cfg(test)]
mod tests;
