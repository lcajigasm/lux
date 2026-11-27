//! Small geometric icons for selector rows. Each one is a few rounded
//! quads, drawn in the same pass as the panel. No icon font is bundled.

use lux_render::QuadInstance;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Icon {
    File,
    Edit,
    View,
    Symbol,
    Command,
    Snippet,
    /// The find bar's in-selection toggle: a block beside two lines.
    Selection,
    /// The active choice in a picker, e.g. the current color scheme.
    Check,
}

fn mark(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
    radius: f32,
) {
    quads.push(QuadInstance::rounded(
        [x, y, w, h],
        super::paint::faded(color),
        radius,
    ));
}

pub(crate) fn push_icon(
    icon: Icon,
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    size: f32,
    ink: [f32; 4],
    hole: [f32; 4],
) {
    let u = size / 14.0;
    match icon {
        Icon::File => mark(quads, x + 3.0 * u, y + u, 8.0 * u, 12.0 * u, ink, 1.5 * u),
        Icon::Edit => {
            for i in 0..3 {
                mark(
                    quads,
                    x + 2.0 * u,
                    y + (3.0 + i as f32 * 3.5) * u,
                    10.0 * u,
                    1.6 * u,
                    ink,
                    0.8 * u,
                );
            }
        }
        Icon::View => {
            mark(quads, x + u, y + 2.0 * u, 12.0 * u, 10.0 * u, ink, 1.5 * u);
            mark(quads, x + 3.0 * u, y + 4.0 * u, 8.0 * u, 6.0 * u, hole, u);
        }
        Icon::Symbol => {
            mark(quads, x + 6.0 * u, y + 2.0 * u, 2.0 * u, 10.0 * u, ink, u);
            mark(quads, x + 2.0 * u, y + 6.0 * u, 10.0 * u, 2.0 * u, ink, u);
        }
        Icon::Command => {
            mark(quads, x + 3.0 * u, y + 3.0 * u, 2.0 * u, 8.0 * u, ink, u);
            mark(quads, x + 5.0 * u, y + 6.0 * u, 6.0 * u, 2.0 * u, ink, u);
        }
        Icon::Snippet => {
            mark(quads, x + 2.0 * u, y + 2.0 * u, 6.0 * u, 10.0 * u, ink, u);
            mark(
                quads,
                x + 9.0 * u,
                y + 4.0 * u,
                3.0 * u,
                2.0 * u,
                ink,
                0.6 * u,
            );
            mark(
                quads,
                x + 9.0 * u,
                y + 8.0 * u,
                3.0 * u,
                2.0 * u,
                ink,
                0.6 * u,
            );
        }
        Icon::Selection => {
            mark(quads, x + 2.0 * u, y + 3.0 * u, 4.0 * u, 4.0 * u, ink, u);
            mark(
                quads,
                x + 8.0 * u,
                y + 4.0 * u,
                4.0 * u,
                1.6 * u,
                ink,
                0.8 * u,
            );
            mark(
                quads,
                x + 8.0 * u,
                y + 8.0 * u,
                4.0 * u,
                1.6 * u,
                ink,
                0.8 * u,
            );
        }
        Icon::Check => {
            // Two strokes of round dots: a short fall, then a long rise.
            let dot = 2.2 * u;
            let path: [(f32, f32); 3] = [(2.0, 7.0), (5.5, 10.5), (12.0, 3.5)];
            for pair in path.windows(2) {
                let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                let steps = ((x1 - x0).abs().max((y1 - y0).abs()) * 2.0).ceil() as usize;
                for step in 0..=steps {
                    let t = step as f32 / steps as f32;
                    let cx = x0 + (x1 - x0) * t;
                    let cy = y0 + (y1 - y0) * t;
                    mark(
                        quads,
                        x + cx * u - dot * 0.5,
                        y + cy * u - dot * 0.5,
                        dot,
                        dot,
                        ink,
                        dot * 0.5,
                    );
                }
            }
        }
    }
}

/// Disclosure triangle of a sidebar folder row, `size` wide, pointing
/// right when collapsed and down when expanded. Built from one-pixel
/// strips (`px` is one physical pixel), so the edges are as sharp as the
/// display allows without a vector path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_disclosure(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    size: f32,
    ink: [f32; 4],
    expanded: bool,
    px: f32,
) {
    let px = px.max(1.0);
    // An isosceles triangle: base `size`, height `size / 2`, centered.
    let height = (size * 0.5).max(px);
    let steps = (height / px).round().max(1.0) as usize;
    for step in 0..steps {
        let along = step as f32 * px;
        let span = (size * (1.0 - along / height)).max(px);
        let inset = (size - span) * 0.5;
        let rect = if expanded {
            [x + inset, y + (size - height) * 0.5 + along, span, px]
        } else {
            [x + (size - height) * 0.5 + along, y + inset, px, span]
        };
        quads.push(QuadInstance::solid(rect, ink));
    }
}

/// Outline of `rect` with lines `line` thick.
fn outline(quads: &mut Vec<QuadInstance>, [x, y, w, h]: [f32; 4], line: f32, ink: [f32; 4]) {
    quads.push(QuadInstance::solid([x, y, w, line], ink));
    quads.push(QuadInstance::solid([x, y + h - line, w, line], ink));
    quads.push(QuadInstance::solid(
        [x, y + line, line, h - 2.0 * line],
        ink,
    ));
    quads.push(QuadInstance::solid(
        [x + w - line, y + line, line, h - 2.0 * line],
        ink,
    ));
}

/// Folder outline in a `size` square: a body with a tab on its top left.
pub(crate) fn push_folder_outline(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    size: f32,
    ink: [f32; 4],
    line: f32,
) {
    let u = size / 14.0;
    let body = [x + u, y + 4.0 * u, 12.0 * u, 8.5 * u];
    outline(quads, body, line, ink);
    quads.push(QuadInstance::solid(
        [x + u, y + 2.5 * u, 5.0 * u, line],
        ink,
    ));
    quads.push(QuadInstance::solid(
        [x + u, y + 2.5 * u, line, 1.5 * u],
        ink,
    ));
    quads.push(QuadInstance::solid(
        [x + 6.0 * u - line, y + 2.5 * u, line, 1.5 * u],
        ink,
    ));
}

/// Document outline in a `size` square: a portrait page, 12 pt tall at
/// a 14 pt slot.
pub(crate) fn push_document_outline(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    size: f32,
    ink: [f32; 4],
    line: f32,
) {
    let u = size / 14.0;
    outline(quads, [x + 3.0 * u, y + u, 8.5 * u, 12.0 * u], line, ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_icon_draws() {
        let ink = [1.0, 0.2, 0.4, 1.0];
        let hole = [0.1, 0.1, 0.1, 1.0];
        for icon in [
            Icon::File,
            Icon::Edit,
            Icon::View,
            Icon::Symbol,
            Icon::Command,
            Icon::Snippet,
            Icon::Selection,
            Icon::Check,
        ] {
            let mut quads = Vec::new();
            push_icon(icon, &mut quads, 0.0, 0.0, 14.0, ink, hole);
            assert!(!quads.is_empty(), "{icon:?} drew nothing");
            assert!(quads.iter().all(|quad| quad.radius >= 0.0));
            // Every mark stays inside its box.
            assert!(quads.iter().all(|quad| quad.rect[0] >= 0.0
                && quad.rect[1] >= 0.0
                && quad.rect[0] + quad.rect[2] <= 14.0
                && quad.rect[1] + quad.rect[3] <= 14.0));
        }
    }
}
