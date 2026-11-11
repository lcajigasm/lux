//! Local branch list. The status bar and `Git: Switch Branch` open it.
//! Enter on a name checks that branch out. Enter on the create row adds
//! a branch at the current commit and leaves the worktree as it is.

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey};

use crate::field::QueryField;

#[derive(Clone)]
pub(crate) struct BranchPicker {
    pub(crate) query: QueryField,
    names: Vec<String>,
    pub(crate) selected_index: usize,
    creating: bool,
}

enum Row {
    Create,
    Branch(String),
}

impl BranchPicker {
    pub(crate) fn new(names: Vec<String>, creating: bool) -> Self {
        Self {
            query: QueryField::new(),
            names,
            selected_index: 0,
            creating,
        }
    }

    pub(crate) fn rows(&self) -> Vec<String> {
        self.row_kinds()
            .into_iter()
            .map(|row| match row {
                Row::Create => self.create_label(),
                Row::Branch(name) => name,
            })
            .collect()
    }

    fn row_kinds(&self) -> Vec<Row> {
        let query = self.query.text().trim();
        let mut rows = Vec::new();
        let exact = self.names.iter().any(|name| name == query);
        if self.creating || query.is_empty() || !exact {
            rows.push(Row::Create);
        }
        let folded = query.to_lowercase();
        for name in &self.names {
            if query.is_empty() || name.to_lowercase().contains(&folded) {
                rows.push(Row::Branch(name.clone()));
            }
        }
        rows
    }

    fn create_label(&self) -> String {
        let query = self.query.text().trim();
        if query.is_empty() {
            "Create new branch…".into()
        } else {
            format!("Create branch {query}")
        }
    }
}

pub(crate) fn handle_branch_picker_key(app: &mut crate::app::App, event: &KeyEvent) -> bool {
    let Some(picker) = app.ui.git.branch_picker.as_ref() else {
        return false;
    };
    let total = picker.rows().len();
    match &event.key {
        Key::Named(NamedKey::Escape) => {
            close(app);
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(picker) = app.ui.git.branch_picker.as_mut()
                && total > 0
            {
                picker.selected_index = (picker.selected_index + 1).min(total - 1);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(picker) = app.ui.git.branch_picker.as_mut() {
                picker.selected_index = picker.selected_index.saturating_sub(1);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.git.branch_picker.as_mut() {
                picker.query.pop();
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            confirm(app);
            true
        }
        Key::Character(text) if !app.modifiers.meta && !app.modifiers.control => {
            let text = event.text.as_deref().unwrap_or(text);
            if !text.chars().any(char::is_control)
                && let Some(picker) = app.ui.git.branch_picker.as_mut()
            {
                picker.query.push_str(text);
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}

pub(crate) fn handle_branch_picker_click(
    app: &mut crate::app::App,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let Some(picker) = app.ui.git.branch_picker.as_ref() else {
        return false;
    };
    let rows = picker.rows();
    let card = list_frame(app, picker.selected_index, rows.len());
    let inside = mouse_x >= card.left
        && mouse_x <= card.left + card.width
        && mouse_y >= card.top
        && mouse_y <= card.top + card.height;
    if !inside {
        close(app);
        return true;
    }
    if mouse_y >= card.rows_top && card.row_height > 0.0 {
        let display = ((mouse_y - card.rows_top) / card.row_height) as usize;
        let index = card.origin + display;
        if display < card.visible && index < rows.len() {
            if let Some(picker) = app.ui.git.branch_picker.as_mut() {
                picker.selected_index = index;
            }
            confirm(app);
        }
    }
    true
}

pub(crate) fn scroll_branch_picker(
    app: &mut crate::app::App,
    delta: lux_platform::ScrollDelta,
) -> bool {
    let Some(picker) = app.ui.git.branch_picker.as_ref() else {
        return false;
    };
    let total = picker.rows().len();
    let steps = crate::overlay_list::wheel_steps(7, delta, app.scale_factor);
    if steps == 0 || total == 0 {
        return true;
    }
    if let Some(picker) = app.ui.git.branch_picker.as_mut() {
        picker.selected_index = picker
            .selected_index
            .saturating_add_signed(steps)
            .min(total - 1);
    }
    app.scheduler.invalidate_all();
    true
}

fn list_frame(
    app: &crate::app::App,
    selected: usize,
    total: usize,
) -> crate::overlay_list::ListFrame {
    let metrics = crate::modal::ModalMetrics::from_app(app);
    crate::overlay_list::centered_frame(
        metrics.vp_width,
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
        selected,
    )
}

fn close(app: &mut crate::app::App) {
    app.ui.git.branch_picker = None;
    crate::focus::focus_editor(app);
    app.scheduler.invalidate_all();
}

fn confirm(app: &mut crate::app::App) {
    let choice = {
        let Some(picker) = app.ui.git.branch_picker.as_ref() else {
            return;
        };
        let rows = picker.row_kinds();
        let row = rows.get(picker.selected_index);
        match row {
            Some(Row::Branch(name)) => Some(Err(name.clone())),
            Some(Row::Create) => {
                let name = picker.query.text().trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(Ok(name))
                }
            }
            None => None,
        }
    };
    app.ui.git.branch_picker = None;
    crate::focus::focus_editor(app);
    match choice {
        Some(Ok(name)) => super::job::request_create_branch(app, name),
        Some(Err(name)) => super::job::request_switch_branch(app, name),
        None => {
            app.ui.announcement = Some("Type a branch name".into());
            app.scheduler.invalidate_all();
        }
    }
}
