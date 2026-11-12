//! Markdown preview painted from the buffer's parse tree.

use swash::FontRef;

use lux_render::QuadInstance;
use lux_syntax::{PreviewBlock, PreviewSpan};

use crate::app::AtlasState;
use crate::render::paint::{color, push_glyph};

#[allow(clippy::too_many_arguments)]
/// Highlight runs for one fenced block. `None` means the fence names no
/// language this build can highlight, and the preview keeps one color.
pub(super) fn fence_runs(
    text: &str,
    info: &str,
    syntax: &lux_syntax::theme::SyntaxTheme,
) -> Option<Vec<Vec<FenceRun>>> {
    let language = fence_language(info)?;
    let mut doc = lux_syntax::Document::new(language);
    doc.set_text(text);
    let tokens = doc.highlight(text);
    let plain = syntax.color_for_scope("text");
    let mut lines = Vec::new();
    let mut offset = 0usize;
    for piece in text.split_inclusive('\n') {
        let body = piece.trim_end_matches(['\n', '\r']);
        let start = offset;
        let end = start + body.len();
        offset += piece.len();
        lines.push(line_runs(text, start, end, &tokens, syntax, plain));
    }
    Some(lines)
}

/// The fence text the preview paints, one block after another.
///
/// Highlight runs keep the bytes between tokens, including spaces and
/// indent. Joining them is the string a reader should be able to copy.
pub(crate) fn rendered_code_blocks(
    source: &str,
    syntax: &lux_syntax::theme::SyntaxTheme,
) -> String {
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
    doc.set_text(source);
    let mut out = String::new();
    for block in lux_syntax::preview_document(&doc, source) {
        let PreviewBlock::Code { info, text } = block else {
            continue;
        };
        if !out.is_empty() {
            out.push('\n');
        }
        match fence_runs(&text, &info, syntax) {
            Some(lines) => {
                for (index, line) in lines.iter().enumerate() {
                    if index > 0 {
                        out.push('\n');
                    }
                    for run in line {
                        out.push_str(&run.text);
                    }
                }
            }
            None => out.push_str(&text),
        }
    }
    out
}

pub(super) struct FenceRun {
    pub text: String,
    pub rgb: (u8, u8, u8),
}

fn fence_language(info: &str) -> Option<lux_syntax::LanguageId> {
    let word = info.split([' ', ',', '\t']).next()?.trim();
    if word.is_empty() {
        return None;
    }
    let word = word.strip_prefix('.').unwrap_or(word);
    if let Some(language) = lux_syntax::LanguageId::from_extension(word) {
        return Some(language);
    }
    let lower = word.to_ascii_lowercase();
    let ext = match lower.as_str() {
        "rust" => "rs",
        "python" | "py" => "py",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "shell" | "bash" | "sh" => "sh",
        "golang" | "go" => "go",
        "c++" | "cpp" => "cpp",
        "ruby" | "rb" => "rb",
        other => other,
    };
    lux_syntax::LanguageId::from_extension(ext)
}

fn line_runs(
    text: &str,
    start: usize,
    end: usize,
    tokens: &[lux_syntax::Token],
    syntax: &lux_syntax::theme::SyntaxTheme,
    plain: lux_syntax::theme::Color,
) -> Vec<FenceRun> {
    let mut runs = Vec::new();
    let mut at = start;
    while at < end {
        if let Some(token) = tokens
            .iter()
            .find(|token| token.start_byte <= at && token.end_byte > at)
        {
            let token_end = token.end_byte.min(end);
            let color = syntax.color_for_token(token);
            runs.push(FenceRun {
                text: text[at..token_end].to_string(),
                rgb: (color.r, color.g, color.b),
            });
            at = token_end;
        } else {
            let next = tokens
                .iter()
                .map(|token| token.start_byte)
                .filter(|start_byte| *start_byte > at && *start_byte < end)
                .min()
                .unwrap_or(end);
            if next > at {
                // The gap is the source bytes the highlighter did not
                // capture. Spaces and indent live here and have to be painted.
                runs.push(FenceRun {
                    text: text[at..next].to_string(),
                    rgb: (plain.r, plain.g, plain.b),
                });
            }
            at = next;
        }
    }
    runs
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_preview(
    buffer: &lux_core::buffer::Buffer,
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    font_size: f32,
    line_height: f32,
    text_left: f32,
    editor_top: f32,
    viewport_height: f32,
    theme: &lux_ui::Theme,
    syntax: &lux_syntax::theme::SyntaxTheme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) {
    let source = buffer.text();
    let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
    doc.set_text(&source);
    let blocks = lux_syntax::preview_document(&doc, &source);
    let family = editor_view.family().to_string();
    let foreground = color(theme.foreground);
    let accent = color(theme.accent);
    let mut y = editor_top + line_height * 0.25;
    let bottom = editor_top + viewport_height;

    for block in blocks {
        if y > bottom {
            break;
        }
        match block {
            PreviewBlock::Heading { level, text } => {
                let size = font_size * (1.55 - f32::from(level.saturating_sub(1)) * 0.08);
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left,
                    y,
                    size,
                    accent,
                    glyph_instances,
                );
                y += size * 1.4 + line_height * 0.35;
            }
            PreviewBlock::Paragraph { spans } => {
                draw_spans(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &spans,
                    text_left,
                    y,
                    font_size,
                    foreground,
                    accent,
                    glyph_instances,
                );
                y += line_height * 1.4;
            }
            PreviewBlock::Code { info, text } => {
                let size = font_size * 0.9;
                if let Some(lines) = fence_runs(&text, &info, syntax) {
                    for line in lines {
                        let mut x = text_left + font_size;
                        for run in line {
                            x += draw_line(
                                editor_view,
                                atlas_state,
                                font,
                                &family,
                                &run.text,
                                x,
                                y,
                                size,
                                [
                                    run.rgb.0 as f32 / 255.0,
                                    run.rgb.1 as f32 / 255.0,
                                    run.rgb.2 as f32 / 255.0,
                                    1.0,
                                ],
                                glyph_instances,
                            );
                        }
                        y += size * 1.3;
                    }
                } else {
                    for line in text.lines() {
                        draw_line(
                            editor_view,
                            atlas_state,
                            font,
                            &family,
                            line,
                            text_left + font_size,
                            y,
                            size,
                            foreground,
                            glyph_instances,
                        );
                        y += size * 1.3;
                    }
                }
                y += line_height * 0.3;
            }
            PreviewBlock::ListItem { text } => {
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &format!("• {text}"),
                    text_left,
                    y,
                    font_size,
                    foreground,
                    glyph_instances,
                );
                y += line_height;
            }
            PreviewBlock::Quote { text } => {
                quad_instances.push(QuadInstance::solid(
                    [text_left, y, 3.0, line_height],
                    accent,
                ));
                draw_line(
                    editor_view,
                    atlas_state,
                    font,
                    &family,
                    &text,
                    text_left + font_size,
                    y,
                    font_size,
                    foreground,
                    glyph_instances,
                );
                y += line_height;
            }
            PreviewBlock::Rule => {
                quad_instances.push(QuadInstance::solid(
                    [text_left, y + line_height * 0.4, 240.0, 1.0],
                    foreground,
                ));
                y += line_height;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_spans(
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    family: &str,
    spans: &[PreviewSpan],
    mut x: f32,
    y: f32,
    font_size: f32,
    foreground: [f32; 4],
    accent: [f32; 4],
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) {
    for span in spans {
        let (text, paint) = match span {
            PreviewSpan::Text(text) => (text.as_str(), foreground),
            PreviewSpan::Strong(text) | PreviewSpan::Link { label: text, .. } => {
                (text.as_str(), accent)
            }
            PreviewSpan::Emphasis(text) | PreviewSpan::Code(text) => (text.as_str(), accent),
        };
        x += draw_line(
            editor_view,
            atlas_state,
            font,
            family,
            text,
            x,
            y,
            font_size,
            paint,
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    editor_view: &mut lux_ui::editor_view::EditorView,
    atlas_state: &mut AtlasState,
    font: FontRef<'_>,
    family: &str,
    text: &str,
    x: f32,
    y: f32,
    font_size: f32,
    paint: [f32; 4],
    glyph_instances: &mut Vec<lux_render::GlyphInstance>,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let style = lux_render::LineStyle {
        family,
        font_size,
        ligatures: false,
    };
    let line = editor_view.text_layout_mut().layout_line(&style, text, &[]);
    for cluster in line.clusters() {
        push_glyph(
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            font_size,
            &cluster,
            x,
            y + line.baseline(),
            paint,
            glyph_instances,
        );
    }
    // `LineLayout::width` drops trailing whitespace. A run that is only
    // the space between `fn` and `main`, or a line's indent, is entirely
    // trailing, so its width is 0 and the next token is painted against
    // the previous one. The cluster end is the advance the caret already
    // uses.
    painted_advance(&line)
}

fn painted_advance(line: &lux_render::LineLayout) -> f32 {
    line.clusters()
        .iter()
        .fold(0.0_f32, |end, cluster| end.max(cluster.x + cluster.advance))
}

#[cfg(test)]
mod tests {
    use super::fence_runs;

    #[test]
    fn a_rust_fence_colors_fn_as_the_monokai_keyword() {
        let syntax = lux_syntax::theme::by_name("Monokai").expect("monokai");
        let lines = fence_runs("fn main() {}\n", "rust", syntax).expect("rust fence");
        let keyword = lines
            .iter()
            .flatten()
            .find(|run| run.text == "fn")
            .expect("fn token");
        assert_eq!(keyword.rgb, (0xF9, 0x26, 0x72));
        assert!(fence_runs("fn main() {}\n", "", syntax).is_none());
    }

    #[test]
    fn a_fence_keeps_spaces_and_indent_when_the_runs_are_painted() {
        let syntax = lux_syntax::theme::by_name("Monokai").expect("monokai");
        let source = include_str!("fixtures/fences.md");
        let mut doc = lux_syntax::Document::new(lux_syntax::LanguageId::Markdown);
        doc.set_text(source);
        let codes: Vec<_> = lux_syntax::preview_document(&doc, source)
            .into_iter()
            .filter_map(|block| match block {
                lux_syntax::PreviewBlock::Code { info, text } => Some((info, text)),
                _ => None,
            })
            .collect();
        assert_eq!(codes.len(), 3, "rust, javascript, and python fences");
        for (info, text) in &codes {
            let lines = text.lines().count();
            assert!(lines >= 8, "{info} has {lines} lines:\n{text}");
            assert!(text.contains("    "), "{info} dropped its indent:\n{text}");
        }
        let painted = super::rendered_code_blocks(source, syntax);
        for needle in [
            "fn main() {",
            "function main() {",
            "def main():",
            "    let name",
            "    const name",
            "    name =",
        ] {
            assert!(
                painted.contains(needle),
                "painted fence lost {needle:?}:\n{painted}"
            );
        }
        assert!(
            !painted.contains("fnmain")
                && !painted.contains("functionmain")
                && !painted.contains("defmain"),
            "highlight runs dropped the spaces: {painted:?}"
        );

        let lines = super::fence_runs(&codes[0].1, "rust", syntax).expect("rust fence");
        let mut cx = lux_render::TextLayoutContext::new();
        let family = cx
            .register_font(include_bytes!(
                "../../../lux-render/tests/fixtures/ahem.ttf"
            ))
            .expect("ahem");
        let style = lux_render::LineStyle {
            family: &family,
            font_size: 16.0,
            ligatures: false,
        };
        for line in lines {
            let text: String = line.iter().map(|run| run.text.as_str()).collect();
            if text.is_empty() {
                continue;
            }
            let whole = cx.layout_line(&style, &text, &[]);
            let mut sum = 0.0_f32;
            for run in &line {
                if run.text.chars().all(char::is_whitespace) && !run.text.is_empty() {
                    let laid = cx.layout_line(&style, &run.text, &[]);
                    assert!(
                        super::painted_advance(&laid) > laid.width(),
                        "a whitespace run must advance past LineLayout::width, which drops it: {text:?}"
                    );
                }
                if run.text.is_empty() {
                    continue;
                }
                let laid = cx.layout_line(&style, &run.text, &[]);
                sum += super::painted_advance(&laid);
            }
            let advance = super::painted_advance(&whole);
            assert!(
                (sum - advance).abs() < 0.5,
                "painted runs lost whitespace on {text:?}: runs {sum}, line {advance}"
            );
            if text.chars().any(|ch| ch == ' ' || ch == '\t') {
                assert!(advance > 0.0, "indent or spaces painted at x=0: {text:?}");
            }
        }
    }
}
