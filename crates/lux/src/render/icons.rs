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

/// Folder or file mark for a sidebar row. The left slot is the disclosure
/// on a directory and empty on a file, so names share one column. These
/// quads are not under the selector fade.
pub(crate) fn push_tree_icon(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    size: f32,
    ink: [f32; 4],
    directory: bool,
    expanded: bool,
) {
    let u = size / 22.0;
    if directory {
        if expanded {
            mark_solid(quads, x + u, y + 8.0 * u, 8.0 * u, 1.8 * u, ink, 0.4 * u);
            mark_solid(
                quads,
                x + 2.4 * u,
                y + 10.2 * u,
                5.2 * u,
                1.8 * u,
                ink,
                0.4 * u,
            );
            mark_solid(
                quads,
                x + 3.8 * u,
                y + 12.4 * u,
                2.4 * u,
                1.8 * u,
                ink,
                0.4 * u,
            );
        } else {
            mark_solid(
                quads,
                x + 2.0 * u,
                y + 6.0 * u,
                1.8 * u,
                8.0 * u,
                ink,
                0.4 * u,
            );
            mark_solid(
                quads,
                x + 4.2 * u,
                y + 7.4 * u,
                1.8 * u,
                5.2 * u,
                ink,
                0.4 * u,
            );
            mark_solid(
                quads,
                x + 6.4 * u,
                y + 8.8 * u,
                1.8 * u,
                2.4 * u,
                ink,
                0.4 * u,
            );
        }
    }
    let body_x = x + 10.0 * u;
    if directory {
        mark_solid(quads, body_x, y + 6.0 * u, 5.0 * u, 2.2 * u, ink, 0.6 * u);
        mark_solid(quads, body_x, y + 7.8 * u, 8.0 * u, 6.5 * u, ink, 0.8 * u);
    } else {
        mark_solid(
            quads,
            body_x + u,
            y + 4.0 * u,
            6.5 * u,
            12.0 * u,
            ink,
            0.8 * u,
        );
    }
}

fn mark_solid(
    quads: &mut Vec<QuadInstance>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: [f32; 4],
    radius: f32,
) {
    quads.push(QuadInstance::rounded([x, y, w, h], color, radius));
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
