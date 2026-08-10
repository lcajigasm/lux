//! Editor widgets, layout, and interaction — the only crate allowed to
//! know about both the domain crates (`lux-core` et al.) and `lux-render`/
//! `lux-platform` (see `docs/ARCHITECTURE.md`).

pub mod accessibility;
pub mod command_palette;
pub mod editor_view;
pub mod file_tree;
pub mod goto_anything;
pub mod minimap;
pub mod search_bar;
pub mod status_bar;
pub mod tabs;
pub mod theme;

pub use accessibility::build_tree as build_accessibility_tree;
pub use editor_view::{
    CursorGlyph, EditorView, EditorViewFrame, EditorViewMetrics, SelectionRect, Viewport,
    VisibleLine,
};
pub use theme::{Color, Theme};
