//! On-save transformations and manual whitespace operations (Sublime Parity ST17).

use std::path::PathBuf;

use lux_core::multicursor::MultiCursor;
use lux_core::selection::Selection;

use super::io::recovery_store;
use crate::app::App;

impl App {
    /// Applies on-save transformations according to user/workspace settings:
    /// - `editor.trimTrailingWhitespace` / `trim_trailing_white_space_on_save`
    /// - `editor.insertFinalNewline` / `ensure_newline_at_eof_on_save`
    /// - `editor.trimTrailingWhitespacePreserveCursor`
    pub(crate) fn apply_on_save_transforms(&mut self) {
        if self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }

        let (trim_ws, insert_nl, preserve_cursor) = {
            let config = self.ui.settings.config.lock().unwrap();
            let r = config.resolved();
            let trim = r["editor.trimTrailingWhitespace"]
                .as_bool()
                .or_else(|| r["trim_trailing_white_space_on_save"].as_bool())
                .or_else(|| r["files.trimTrailingWhitespace"].as_bool())
                .unwrap_or(false);
            let nl = r["editor.insertFinalNewline"]
                .as_bool()
                .or_else(|| r["ensure_newline_at_eof_on_save"].as_bool())
                .or_else(|| r["files.insertFinalNewline"].as_bool())
                .unwrap_or(true);
            let preserve = r["editor.trimTrailingWhitespacePreserveCursor"]
                .as_bool()
                .or_else(|| r["trim_trailing_white_space_on_save_preserve_cursor"].as_bool())
                .unwrap_or(false);
            (trim, nl, preserve)
        };

        let mut all_edits = Vec::new();
        let before_selections: Vec<Selection> = self.cursors.cursors().to_vec();

        if trim_ws {
            let cursor_lines: Vec<usize> = if preserve_cursor {
                self.cursors.cursors().iter().map(|s| s.head.line).collect()
            } else {
                Vec::new()
            };
            let preserve_slice = if preserve_cursor {
                Some(cursor_lines.as_slice())
            } else {
                None
            };
            let trim_edits =
                lux_core::edit_ops::trim_trailing_whitespace(&mut self.buffer, preserve_slice);
            all_edits.extend(trim_edits);
        }

        if insert_nl {
            let nl_edits = lux_core::edit_ops::ensure_final_newline(&mut self.buffer);
            all_edits.extend(nl_edits);
        }

        if !all_edits.is_empty() {
            self.finalize_whitespace_edits(all_edits, before_selections);
        }
    }

    /// Trims trailing whitespace manually (on demand).
    pub(crate) fn trim_trailing_whitespace(&mut self) {
        if self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        let before_selections: Vec<Selection> = self.cursors.cursors().to_vec();
        let edits = lux_core::edit_ops::trim_trailing_whitespace(&mut self.buffer, None);
        if !edits.is_empty() {
            self.finalize_whitespace_edits(edits, before_selections);
        }
    }

    /// Ensures final newline manually (on demand).
    pub(crate) fn ensure_final_newline(&mut self) {
        if self.buffer.is_huge_file_mode() || self.editing_blocked() {
            return;
        }
        let before_selections: Vec<Selection> = self.cursors.cursors().to_vec();
        let edits = lux_core::edit_ops::ensure_final_newline(&mut self.buffer);
        if !edits.is_empty() {
            self.finalize_whitespace_edits(edits, before_selections);
        }
    }

    fn finalize_whitespace_edits(
        &mut self,
        edits: Vec<lux_core::edit::Edit>,
        before_selections: Vec<Selection>,
    ) {
        let after_selections: Vec<Selection> = before_selections
            .iter()
            .map(|s| {
                let anchor = lux_core::movement::position_at_line_column(
                    &self.buffer,
                    s.anchor.line,
                    s.anchor.column,
                );
                let head = lux_core::movement::position_at_line_column(
                    &self.buffer,
                    s.head.line,
                    s.head.column,
                );
                Selection::new(anchor, head)
            })
            .collect();

        self.cursors = MultiCursor::from_cursors(after_selections.clone());
        self.reset_goal_columns();
        self.sync_syntax_incremental(&edits);
        let tx = lux_core::history::Transaction::new(edits, before_selections, after_selections);
        self.history.record(tx);
        self.scheduler.invalidate_all();
    }

    /// Updates a setting key-value pair, saves to disk, and reloads user configuration.
    #[allow(dead_code)]
    pub(crate) fn update_setting(&mut self, key: &str, val: serde_json::Value) {
        let path = self.ui.settings.settings_path.clone();
        let mut map: serde_json::Map<String, serde_json::Value> =
            match std::fs::read_to_string(&path) {
                Ok(s) => lux_config::layering::jsonc::parse(&s)
                    .ok()
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default(),
                Err(_) => serde_json::Map::new(),
            };

        map.insert(key.to_string(), val);
        let obj = serde_json::Value::Object(map);
        if let Ok(json_str) = serde_json::to_string_pretty(&obj) {
            let _ = std::fs::write(&path, json_str);
        }

        let _ = self
            .ui
            .settings
            .config
            .lock()
            .unwrap()
            .reload_layer(lux_config::layering::Layer::User);
    }

    pub(crate) fn save_recovery_snapshot(&self) {
        if self.document_dirty()
            && let Some(path) = &self.documents.path
            && !self.buffer.is_huge_file_mode()
            && let Some(store) = recovery_store()
        {
            let _ = store.save(path, &self.buffer.text());
        }
    }

    pub(crate) fn recoverable_documents() -> Vec<PathBuf> {
        if let Some(store) = recovery_store()
            && let Ok(recoverable) = store.recoverable()
        {
            recoverable.into_iter().map(|s| s.path).collect()
        } else {
            Vec::new()
        }
    }
}
