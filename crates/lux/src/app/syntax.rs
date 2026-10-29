//! Incremental Tree-sitter syntax highlighting synchronization.
//!
//! Offloads syntax parsing to `BackgroundHighlighter` on a worker thread via
//! non-blocking channels, maintaining cached tokens and fold ranges on the UI
//! thread for immediate, 120 FPS latency.

use lux_core::buffer::Buffer;
use lux_core::edit::Edit;
use lux_core::folding::FoldRange;
use lux_syntax::{InputEdit, Point};

use super::App;
use super::types::SyntaxState;

/// Computes the Tree-sitter `InputEdit` for an `Edit` that was just applied to `buffer`.
pub(crate) fn compute_input_edit(buffer: &Buffer, edit: &Edit) -> InputEdit {
    match edit {
        Edit::Insert { char_idx, text } => {
            let start_char = *char_idx;
            let start_byte = buffer.char_to_byte(start_char);
            let char_count = text.chars().count();
            let new_end_byte = buffer.char_to_byte(start_char + char_count);
            let old_end_byte = start_byte;

            let start_row = buffer.byte_to_line(start_byte);
            let start_col = start_byte - buffer.line_to_byte(start_row);
            let start_position = Point {
                row: start_row,
                column: start_col,
            };
            let old_end_position = start_position;

            let new_end_row = buffer.byte_to_line(new_end_byte);
            let new_end_col = new_end_byte - buffer.line_to_byte(new_end_row);
            let new_end_position = Point {
                row: new_end_row,
                column: new_end_col,
            };

            InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            }
        }
        Edit::Delete { char_idx, text } => {
            let start_char = *char_idx;
            let start_byte = buffer.char_to_byte(start_char);
            let new_end_byte = start_byte;
            let old_end_byte = start_byte + text.len();

            let start_row = buffer.byte_to_line(start_byte);
            let start_col = start_byte - buffer.line_to_byte(start_row);
            let start_position = Point {
                row: start_row,
                column: start_col,
            };
            let new_end_position = start_position;

            let newlines = text.bytes().filter(|&b| b == b'\n').count();
            let old_end_row = start_row + newlines;
            let old_end_col = if newlines == 0 {
                start_col + text.len()
            } else {
                let last_line = text.rsplit('\n').next().unwrap_or("");
                last_line.len()
            };
            let old_end_position = Point {
                row: old_end_row,
                column: old_end_col,
            };

            InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position,
                old_end_position,
                new_end_position,
            }
        }
    }
}

/// Shifts cached syntax tokens in response to an edit, preserving colors during active typing.
pub(crate) fn shift_tokens(tokens: &mut Vec<lux_syntax::Token>, edit: &InputEdit) {
    let start_byte = edit.start_byte;
    let old_end_byte = edit.old_end_byte;
    let new_end_byte = edit.new_end_byte;
    let delta = new_end_byte as isize - old_end_byte as isize;

    let start_idx = tokens.partition_point(|t| t.end_byte <= start_byte);
    let mut write_idx = start_idx;
    for read_idx in start_idx..tokens.len() {
        let token = &mut tokens[read_idx];
        if token.start_byte >= old_end_byte {
            token.start_byte = (token.start_byte as isize + delta).max(0) as usize;
            token.end_byte =
                (token.end_byte as isize + delta).max(token.start_byte as isize) as usize;
        } else {
            let new_end = (token.end_byte as isize + delta).max(token.start_byte as isize) as usize;
            token.end_byte = new_end;
        }
        if token.end_byte > token.start_byte {
            if write_idx != read_idx {
                tokens.swap(write_idx, read_idx);
            }
            write_idx += 1;
        }
    }
    tokens.truncate(write_idx);
}

impl App {
    /// Initializes syntax state and starts the background highlighter when a document is opened.
    pub(crate) fn update_syntax_on_open(&mut self) {
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        let language = self.documents.language().or_else(|| {
            crate::document::detect_language(self.documents.path.as_deref(), &self.buffer)
        });
        if let Some(lang) = language {
            self.documents.set_language(Some(lang));
            let rope = self.buffer.rope().clone();
            let highlighter = lux_syntax::BackgroundHighlighter::spawn(lang, None);
            highlighter.submit(lux_syntax::Job::SetRope { rope, version: 0 });
            let initial_res = highlighter.recv();
            let (tokens, fold_ranges) = match initial_res {
                Some(res) => {
                    let ranges: Vec<FoldRange> = res
                        .fold_ranges
                        .into_iter()
                        .map(|r| FoldRange::new(r.start_row, r.end_row))
                        .collect();
                    (res.tokens, ranges)
                }
                None => (Vec::new(), Vec::new()),
            };
            self.syntax = Some(SyntaxState {
                highlighter,
                language: lang,
                theme: self.active_syntax_theme(),
                tokens,
                fold_ranges: fold_ranges.clone(),
                version: 0,
            });
            self.fold_state.set_foldable_ranges(fold_ranges);
        } else {
            self.syntax = None;
        }
        self.refresh_foldable_ranges();
    }

    /// Sets the language mode explicitly (e.g. from the status bar or command palette).
    pub(crate) fn set_language_mode(&mut self, language: Option<lux_syntax::LanguageId>) {
        self.documents.set_language(language);
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        if let Some(lang) = language {
            let rope = self.buffer.rope().clone();
            let highlighter = lux_syntax::BackgroundHighlighter::spawn(lang, None);
            highlighter.submit(lux_syntax::Job::SetRope { rope, version: 0 });
            let initial_res = highlighter.recv();
            let (tokens, fold_ranges) = match initial_res {
                Some(res) => {
                    let ranges: Vec<FoldRange> = res
                        .fold_ranges
                        .into_iter()
                        .map(|r| FoldRange::new(r.start_row, r.end_row))
                        .collect();
                    (res.tokens, ranges)
                }
                None => (Vec::new(), Vec::new()),
            };
            self.syntax = Some(SyntaxState {
                highlighter,
                language: lang,
                theme: self.active_syntax_theme(),
                tokens,
                fold_ranges: fold_ranges.clone(),
                version: 0,
            });
            self.fold_state.set_foldable_ranges(fold_ranges);
            let name = crate::status_bar::language_name(Some(lang));
            crate::accessibility::announce(self, format!("Language mode set to {name}"));
        } else {
            self.syntax = None;
            crate::accessibility::announce(self, "Language mode set to Plain Text");
        }
        self.refresh_foldable_ranges();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Incremental syntax synchronization: sends only edit deltas and O(1) Rope snapshots
    /// to the worker thread without materializing the full text.
    pub(crate) fn sync_syntax_incremental(&mut self, edits: &[Edit]) {
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        if edits.is_empty() {
            return;
        }

        syntax.version += 1;
        let version = syntax.version;

        let mut input_edits = Vec::with_capacity(edits.len());
        for edit in edits {
            let input_edit = compute_input_edit(&self.buffer, edit);
            shift_tokens(&mut syntax.tokens, &input_edit);
            input_edits.push(input_edit);
        }

        syntax.highlighter.submit(lux_syntax::Job::Edit {
            edits: input_edits,
            rope: self.buffer.rope().clone(),
            version,
        });

        if let Some(path) = self.documents.path.clone()
            && (self.ui.settings.is_settings_path(&path) || self.ui.lsp.sync.is_some())
        {
            crate::lsp::sync_document_change(self, &path, &self.buffer.text());
        }
    }

    /// Full rope synchronization without string allocation (e.g. on undo/redo).
    pub(crate) fn sync_syntax_full(&mut self) {
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        syntax.version += 1;
        let version = syntax.version;
        syntax.highlighter.submit(lux_syntax::Job::SetRope {
            rope: self.buffer.rope().clone(),
            version,
        });

        self.poll_syntax_highlighter();

        if let Some(path) = self.documents.path.clone()
            && (self.ui.settings.is_settings_path(&path) || self.ui.lsp.sync.is_some())
        {
            crate::lsp::sync_document_change(self, &path, &self.buffer.text());
        }
    }

    /// Polls the background highlighter for newly parsed results without blocking.
    pub(crate) fn poll_syntax_highlighter(&mut self) {
        let Some(syntax) = &mut self.syntax else {
            return;
        };
        if let Some(res) = syntax.highlighter.try_recv()
            && res.version >= syntax.version
        {
            syntax.tokens = res.tokens;
            let ranges: Vec<FoldRange> = res
                .fold_ranges
                .into_iter()
                .map(|r| FoldRange::new(r.start_row, r.end_row))
                .collect();
            syntax.fold_ranges = ranges.clone();
            self.fold_state.set_foldable_ranges(ranges);
            self.scheduler.invalidate_all();
            if let Some(gpu) = &self.gpu_window {
                gpu.window.request_redraw();
            }
        }
    }
}
