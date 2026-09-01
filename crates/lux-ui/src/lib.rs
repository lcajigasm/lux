//! Editor widgets, layout, and interaction — the only crate allowed to
//! know about both the domain crates (`lux-core` et al.) and `lux-render`/
//! `lux-platform` (see `docs/ARCHITECTURE.md`).

pub mod accessibility;
pub mod chrome;
pub mod command_palette;
pub mod editor_view;
pub mod file_tree;
pub mod focus;
pub mod goto_anything;
pub mod i18n;
pub mod minimap;
pub mod motion;
pub mod rtl;
pub mod search_bar;
pub mod status_bar;
pub mod surfaces;
pub mod tabs;
pub mod theme;

pub use accessibility::build_tree as build_accessibility_tree;
pub use chrome::Chrome;
pub use editor_view::{
    CursorGlyph, EditorView, EditorViewFrame, EditorViewMetrics, SelectionRect, Viewport,
    VisibleLine,
};
pub use focus::{FocusRing, FocusTarget};
pub use i18n::Locale;
pub use motion::Motion;
pub use rtl::{Direction, direction_for};
pub use theme::{Color, Theme, contrast_ratio};
