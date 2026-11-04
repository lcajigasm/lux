//! All editor geometry and GPU uniforms use physical pixels. Convert the
//! logical defaults together whenever the platform reports a new scale.

use crate::app::{App, FONT_SIZE, LINE_HEIGHT};

impl App {
    pub(crate) fn set_display_scale(&mut self, scale: f64) {
        let scale = scale as f32;
        if !scale.is_finite() || scale <= 0.0 || scale == self.scale_factor {
            return;
        }
        let ratio = scale / self.scale_factor;
        self.scale_factor = scale;
        self.viewport.scroll_x *= ratio;
        self.viewport.scroll_y *= ratio;
        self.editor_view
            .set_text_dimensions(FONT_SIZE * scale, LINE_HEIGHT * scale);
        self.scheduler.invalidate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_core::buffer::Buffer;
    use std::time::Duration;

    #[test]
    fn retina_scaling_preserves_logical_viewport_and_doubles_glyph_geometry() {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes("text\n".repeat(100).as_bytes()).unwrap();
        app.viewport.scroll_y = LINE_HEIGHT * 10.0;
        app.viewport.scroll_x = 7.0;
        app.viewport.height = LINE_HEIGHT * 4.0;
        let first = app.editor_view.build_frame(
            &app.buffer,
            &app.cursors,
            &app.viewport,
            Duration::ZERO,
            Some(&app.fold_state),
        );
        app.set_display_scale(2.0);
        app.viewport.height *= 2.0;
        let retina = app.editor_view.build_frame(
            &app.buffer,
            &app.cursors,
            &app.viewport,
            Duration::ZERO,
            Some(&app.fold_state),
        );
        assert_eq!(first.visible_line_range, retina.visible_line_range);
        assert!((first.lines[0].layout.width() * 2.0 - retina.lines[0].layout.width()).abs() < 0.1);
        assert_eq!(app.viewport.scroll_x, 14.0);
        assert!(
            (first.lines[0].layout.baseline() * 2.0 - retina.lines[0].layout.baseline()).abs()
                < 0.1
        );
        app.set_display_scale(1.0);
        assert_eq!(app.viewport.scroll_y, LINE_HEIGHT * 10.0);
        app.set_display_scale(f64::NAN);
        assert_eq!(app.scale_factor, 1.0);
    }

    #[test]
    fn page_motion_and_selection_use_the_scaled_line_height() {
        use lux_platform::event::KeyEvent;
        use lux_platform::{ElementState, Key, NamedKey};
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.buffer = Buffer::from_bytes("text\n".repeat(100).as_bytes()).unwrap();
        app.set_display_scale(2.0);
        app.viewport.height = LINE_HEIGHT * 2.0 * 3.0;
        app.modifiers.shift = true;
        app.handle_key(&KeyEvent {
            key: Key::Named(NamedKey::PageDown),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
            physical: None,
        });
        assert_eq!(app.cursors.primary().head.line, 3);
        assert_eq!(app.cursors.primary().anchor.line, 0);
        let frame = app.editor_view.build_frame(
            &app.buffer,
            &app.cursors,
            &app.viewport,
            Duration::ZERO,
            Some(&app.fold_state),
        );
        assert!(
            frame
                .cursors
                .iter()
                .any(|cursor| cursor.primary && cursor.line == 3)
        );
    }
}
