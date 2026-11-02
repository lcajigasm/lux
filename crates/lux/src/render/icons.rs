//! Small geometric icons for selector rows. Each one is a few rounded
//! quads, drawn in the same pass as the panel. No icon font is bundled.

use lux_render::QuadInstance;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Icon {
    File,
    Folder,
    Edit,
    Find,
    Selection,
    Goto,
    View,
    Git,
    Terminal,
    Settings,
    Theme,
    Symbol,
    Command,
    Build,
    Snippet,
    Help,
}

pub(crate) fn icon_for_category(category: &str) -> Icon {
    match category {
        "File" => Icon::File,
        "Edit" => Icon::Edit,
        "Find" => Icon::Find,
        "Selection" => Icon::Selection,
        "Goto" => Icon::Goto,
        "View" => Icon::View,
        "Git" => Icon::Git,
        "LSP" => Icon::Symbol,
        "Terminal" => Icon::Terminal,
        "Preferences" | "UI" => Icon::Settings,
        "Build" => Icon::Build,
        "Project" => Icon::Folder,
        "Snippets" => Icon::Snippet,
        "Help" => Icon::Help,
        _ => Icon::Command,
    }
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
        Icon::Folder => {
            mark(quads, x + u, y + 3.0 * u, 6.0 * u, 3.0 * u, ink, u);
            mark(quads, x + u, y + 5.0 * u, 12.0 * u, 8.0 * u, ink, 1.5 * u);
        }
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
        Icon::Find => {
            mark(
                quads,
                x + 2.0 * u,
                y + 2.0 * u,
                8.0 * u,
                8.0 * u,
                ink,
                4.0 * u,
            );
            mark(
                quads,
                x + 4.0 * u,
                y + 4.0 * u,
                4.0 * u,
                4.0 * u,
                hole,
                2.0 * u,
            );
            mark(
                quads,
                x + 9.0 * u,
                y + 9.0 * u,
                3.0 * u,
                3.0 * u,
                ink,
                0.5 * u,
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
        Icon::Goto => {
            mark(quads, x + 2.0 * u, y + 6.0 * u, 8.0 * u, 2.0 * u, ink, u);
            mark(quads, x + 8.0 * u, y + 3.0 * u, 2.0 * u, 8.0 * u, ink, u);
        }
        Icon::View => {
            mark(quads, x + u, y + 2.0 * u, 12.0 * u, 10.0 * u, ink, 1.5 * u);
            mark(quads, x + 3.0 * u, y + 4.0 * u, 8.0 * u, 6.0 * u, hole, u);
        }
        Icon::Git => {
            mark(
                quads,
                x + 2.0 * u,
                y + 2.0 * u,
                4.0 * u,
                4.0 * u,
                ink,
                2.0 * u,
            );
            mark(quads, x + 5.0 * u, y + 4.0 * u, 2.0 * u, 6.0 * u, ink, u);
            mark(quads, x + 5.0 * u, y + 8.0 * u, 6.0 * u, 2.0 * u, ink, u);
            mark(
                quads,
                x + 9.0 * u,
                y + 7.0 * u,
                3.0 * u,
                3.0 * u,
                ink,
                1.5 * u,
            );
        }
        Icon::Terminal => {
            mark(quads, x + u, y + 3.0 * u, 12.0 * u, 8.0 * u, ink, 1.5 * u);
            mark(
                quads,
                x + 3.0 * u,
                y + 6.0 * u,
                3.0 * u,
                2.0 * u,
                hole,
                0.4 * u,
            );
            mark(
                quads,
                x + 7.0 * u,
                y + 6.0 * u,
                3.0 * u,
                2.0 * u,
                hole,
                0.4 * u,
            );
        }
        Icon::Settings => {
            mark(
                quads,
                x + 2.0 * u,
                y + 2.0 * u,
                10.0 * u,
                10.0 * u,
                ink,
                2.0 * u,
            );
            mark(
                quads,
                x + 5.0 * u,
                y + 5.0 * u,
                4.0 * u,
                4.0 * u,
                hole,
                2.0 * u,
            );
        }
        Icon::Theme => {
            let c = size * 0.38;
            mark(quads, x + u, y + u, c, c, ink, u);
            mark(quads, x + u + c + u, y + u, c, c, hole, u);
            mark(quads, x + u, y + u + c + u, c, c, hole, u);
            mark(quads, x + u + c + u, y + u + c + u, c, c, ink, u);
        }
        Icon::Symbol => {
            mark(quads, x + 6.0 * u, y + 2.0 * u, 2.0 * u, 10.0 * u, ink, u);
            mark(quads, x + 2.0 * u, y + 6.0 * u, 10.0 * u, 2.0 * u, ink, u);
        }
        Icon::Command => {
            mark(quads, x + 3.0 * u, y + 3.0 * u, 2.0 * u, 8.0 * u, ink, u);
            mark(quads, x + 5.0 * u, y + 6.0 * u, 6.0 * u, 2.0 * u, ink, u);
        }
        Icon::Build => {
            mark(quads, x + 2.0 * u, y + 10.0 * u, 10.0 * u, 2.0 * u, ink, u);
            mark(quads, x + 6.0 * u, y + 2.0 * u, 2.0 * u, 8.0 * u, ink, u);
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
        Icon::Help => mark(
            quads,
            x + 3.0 * u,
            y + 2.0 * u,
            8.0 * u,
            10.0 * u,
            ink,
            4.0 * u,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_pick_distinct_icons_and_each_icon_draws() {
        assert_eq!(icon_for_category("File"), Icon::File);
        assert_eq!(icon_for_category("Find"), Icon::Find);
        assert_ne!(icon_for_category("File"), icon_for_category("Git"));
        let ink = [1.0, 0.2, 0.4, 1.0];
        let hole = [0.1, 0.1, 0.1, 1.0];
        for icon in [
            Icon::File,
            Icon::Folder,
            Icon::Edit,
            Icon::Find,
            Icon::Selection,
            Icon::Goto,
            Icon::View,
            Icon::Git,
            Icon::Terminal,
            Icon::Settings,
            Icon::Theme,
            Icon::Symbol,
            Icon::Command,
            Icon::Build,
            Icon::Snippet,
            Icon::Help,
        ] {
            let mut quads = Vec::new();
            push_icon(icon, &mut quads, 0.0, 0.0, 14.0, ink, hole);
            assert!(!quads.is_empty(), "{icon:?} drew nothing");
            assert!(quads.iter().all(|quad| quad.radius >= 0.0));
        }
    }
}
