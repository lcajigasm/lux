//! The editor of a window without splits: background, current line, text,
//! find matches, carets, the gutter, and the Markdown or HTML preview
//! beside the text when it is open.

use lux_core::bookmarks::BookmarkState;
use lux_core::buffer::Buffer;
use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::editor_view::{EditorView, EditorViewFrame, Viewport};
use swash::FontRef;

use super::paint::color;
use super::{html_view, search, text};
use crate::app::{AtlasState, SyntaxState};
use crate::search::SearchBarState;

/// What the single editor pane paints from.
pub(super) struct SinglePane<'a> {
    pub frame: &'a EditorViewFrame,
    pub buffer: &'a Buffer,
    pub syntax: Option<&'a SyntaxState>,
    /// Styling of the `Find Results` tab, when it is the active buffer.
    pub find_decor: Option<&'a text::find_decor::FindDecor>,
    pub viewport: &'a Viewport,
    pub bookmarks: &'a BookmarkState,
    pub documents: &'a crate::document::Documents,
    pub search: Option<&'a SearchBarState>,
    pub syntax_theme: &'a lux_syntax::theme::SyntaxTheme,
    pub theme: &'a lux_ui::Theme,
    pub font: FontRef<'static>,
    pub font_size: f32,
    pub line_height: f32,
    pub stroke: f32,
    pub scale_factor: f32,
    pub editor_top: f32,
    pub sidebar_width: f32,
    pub text_left: f32,
    pub gutter_total_width: f32,
    pub window_width: f32,
    pub distraction_free: bool,
    /// See `MouseState::gutter_hover`.
    pub gutter_hover: Option<[f32; 2]>,
}

/// Where the pane's instances go. The gutter has its own lists because it
/// paints over the text.
pub(super) struct PaneOutput<'a> {
    pub quads: &'a mut Vec<QuadInstance>,
    pub glyphs: &'a mut Vec<GlyphInstance>,
    pub gutter_quads: &'a mut Vec<QuadInstance>,
    pub gutter_glyphs: &'a mut Vec<GlyphInstance>,
}

impl SinglePane<'_> {
    /// Paints the pane and returns where the primary caret is on screen.
    pub(super) fn paint(
        &self,
        editor_view: &mut EditorView,
        atlas_state: &mut AtlasState,
        out: &mut PaneOutput<'_>,
    ) -> Option<(f32, f32)> {
        let &SinglePane {
            frame,
            buffer,
            viewport,
            theme,
            font,
            font_size,
            line_height,
            stroke,
            editor_top,
            sidebar_width,
            text_left,
            window_width,
            ..
        } = self;
        let is_df = self.distraction_free;
        let documents = self.documents;
        let quad_instances = &mut *out.quads;
        let glyph_instances = &mut *out.glyphs;
        quad_instances.push(QuadInstance::solid(
            [sidebar_width, editor_top, viewport.width, viewport.height],
            color(theme.background),
        ));

        let html_preview = documents.active_entry().is_some_and(|entry| {
            entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Html)
        });
        let show_preview = documents.active_entry().is_some_and(|entry| {
            entry.markdown_preview && entry.language == Some(lux_syntax::LanguageId::Markdown)
        });
        let content_width = (viewport.width - sidebar_width).max(1.0);
        let pane_cap = (content_width * 0.62).max(1.0);
        let preview_width = if html_preview || show_preview {
            (content_width * 0.46).clamp(180.0_f32.min(pane_cap), pane_cap)
        } else {
            0.0
        };
        let mut editor_viewport = *viewport;
        if html_preview || show_preview {
            editor_viewport.width = (content_width - preview_width).max(1.0);
        }
        for cursor in &frame.cursors {
            if cursor.primary {
                let cur_y = editor_top + cursor.y - viewport.scroll_y;
                if cur_y >= editor_top && cur_y < editor_top + viewport.height {
                    let h_x = if is_df { 0.0 } else { text_left };
                    let h_w = if is_df {
                        window_width
                    } else {
                        (sidebar_width + editor_viewport.width - text_left).max(0.0)
                    };
                    quad_instances.push(QuadInstance::solid(
                        [h_x, cur_y, h_w, line_height],
                        color(theme.chrome.line_highlight),
                    ));
                }
            }
        }
        text::render_text_lines(
            frame,
            buffer,
            self.syntax,
            self.find_decor,
            &editor_viewport,
            editor_top,
            line_height,
            sidebar_width,
            text_left,
            stroke,
            theme,
            atlas_state,
            font,
            font_size,
            quad_instances,
            glyph_instances,
        );
        if let Some(search_state) = self.search {
            let origin = search::TextOrigin {
                viewport: &editor_viewport,
                editor_top,
                sidebar_width,
                text_left,
                line_height,
            };
            search::render_search_matches(
                search_state,
                buffer,
                frame,
                &origin,
                stroke,
                theme,
                quad_instances,
            );
        }
        let line_pad_y = (line_height - font_size) * 0.5;
        let caret_h = font_size + 2.0 * stroke;
        let cursor_screen = text::render_carets(
            frame,
            &editor_viewport,
            editor_top,
            text_left,
            sidebar_width,
            line_pad_y,
            stroke,
            caret_h,
            color(theme.accent),
            quad_instances,
        );
        {
            let marks = text::GutterMarks::new(
                Some(self.bookmarks),
                self.gutter_hover,
                [sidebar_width, editor_top],
            );
            text::render_gutter(
                frame,
                marks,
                &editor_viewport,
                editor_top,
                line_height,
                sidebar_width,
                self.gutter_total_width,
                stroke,
                theme,
                atlas_state,
                font,
                font_size,
                out.gutter_quads,
                out.gutter_glyphs,
            );
        }
        html_view::paint_open(
            show_preview,
            html_preview,
            buffer,
            editor_view,
            atlas_state,
            font,
            font_size,
            line_height,
            sidebar_width + editor_viewport.width,
            preview_width,
            editor_top,
            viewport.height,
            16.0 * self.scale_factor,
            stroke,
            theme,
            self.syntax_theme,
            quad_instances,
            glyph_instances,
        );
        cursor_screen
    }
}
