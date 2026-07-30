//! Editor widgets, layout, and interaction — the only crate allowed to
//! know about both the domain crates (`lux-core` et al.) and `lux-render`/
//! `lux-platform` (see `docs/ARCHITECTURE.md`).

pub mod editor_view;

pub use editor_view::{
    CursorGlyph, EditorView, EditorViewFrame, EditorViewMetrics, SelectionRect, Viewport,
    VisibleLine,
};
