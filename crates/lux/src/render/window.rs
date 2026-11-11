use dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use lux_platform::{PlatformContext, PlatformWindow, ScrollDelta, WindowAttributes};
use lux_render::{GpuContext, QuadPipeline, Surface, TextPipeline};

use crate::app::{ATLAS_SIDE, App, GpuWindow};
use crate::render::paint::color;

impl App {
    pub(crate) fn create_gpu_window(&mut self, ctx: &dyn PlatformContext) {
        let placed = crate::place::restored(ctx);
        let mut attrs = WindowAttributes {
            title: "Lux".to_string(),
            inner_size: Some(LogicalSize::new(1000.0, 700.0)),
            ..Default::default()
        };
        if let Some(frame) = &placed {
            attrs.position = Some(PhysicalPosition::new(frame.x, frame.y));
            if frame.fullscreen {
                attrs.fullscreen = true;
                attrs.inner_size = None;
            } else if frame.maximized {
                attrs.maximized = true;
                attrs.inner_size = None;
            } else {
                attrs.physical_inner_size = Some(PhysicalSize::new(frame.width, frame.height));
                attrs.inner_size = None;
            }
        }
        let window = ctx
            .create_window(attrs)
            .expect("failed to create the main window");
        window.set_ime_allowed(true);
        let window: &'static dyn PlatformWindow = Box::leak(window);

        let size = window.inner_size();
        self.set_display_scale(window.scale_factor());
        eprintln!("Lux: display scale {}", self.scale_factor);
        let phase = std::time::Instant::now();
        let gpu = pollster::block_on(GpuContext::new(None))
            .expect("a real GPU adapter must be available to run the Lux desktop app");
        eprintln!(
            "Lux: phase gpu {:.1} ms",
            phase.elapsed().as_secs_f64() * 1000.0
        );
        let phase = std::time::Instant::now();
        let surface = Surface::new(&gpu.instance, &gpu.adapter, &gpu.device, window, size)
            .expect("failed to create the window's wgpu surface");
        let format = surface.config().format;
        let quads = QuadPipeline::new(&gpu.device, format);
        let text = TextPipeline::new(&gpu.device, format, ATLAS_SIDE);
        eprintln!(
            "Lux: phase pipelines {:.1} ms",
            phase.elapsed().as_secs_f64() * 1000.0
        );
        quads.set_viewport_size(&gpu.queue, size.width, size.height);
        if let Some(scheme) = window.color_scheme() {
            self.apply_system_scheme(scheme);
        }
        text.set_globals(
            &gpu.queue,
            size.width,
            size.height,
            color(self.chrome.theme.background),
        );

        let tab_bar_height = self.tab_bar_height();
        let status_bar_height = self.status_bar_height();
        let term_dock_height = self.term_dock_height();
        self.viewport.width = size.width as f32;
        self.viewport.height =
            (size.height as f32 - tab_bar_height - status_bar_height - term_dock_height).max(1.0);

        let id = window.id();
        self.gpu_window = Some(GpuWindow {
            window,
            id,
            gpu,
            surface,
            quads,
            text,
        });
        self.apply_user_settings();
        self.scheduler.invalidate_all();
        window.request_redraw();
    }

    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.presentation
            .set_hidden(size.width == 0 || size.height == 0);
        let tab_bar_height = self.tab_bar_height();
        let status_bar_height = self.status_bar_height();
        let search_bar_height = self.search_bar_height();
        let term_dock_height = self.term_dock_height();
        let Some(gpu_window) = &mut self.gpu_window else {
            return;
        };
        gpu_window.surface.resize(&gpu_window.gpu.device, size);
        gpu_window
            .quads
            .set_viewport_size(&gpu_window.gpu.queue, size.width, size.height);
        gpu_window.text.set_globals(
            &gpu_window.gpu.queue,
            size.width,
            size.height,
            color(self.chrome.theme.background),
        );
        self.viewport.width = size.width as f32;
        self.viewport.height = (size.height as f32
            - tab_bar_height
            - status_bar_height
            - search_bar_height
            - term_dock_height)
            .max(1.0);
        self.scheduler.invalidate_all();
        gpu_window.window.request_redraw();
    }

    pub(crate) fn sync_editor_clear_color(&mut self) {
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        let size = gpu_window.window.inner_size();
        let clear = color(self.chrome.theme.background);
        gpu_window
            .text
            .set_globals(&gpu_window.gpu.queue, size.width, size.height, clear);
    }

    /// The tab strip takes the wheel only after the pointer has been seen
    /// inside that strip. The origin is the default before any move, and it
    /// sits in the strip's rectangle, so a wheel that arrives first scrolls
    /// the editor.
    pub(crate) fn wheel_targets_tabs(&self) -> bool {
        self.mouse.placed
            && self.mouse.position.y < self.tab_bar_height() as f64
            && self.mouse.position.x >= self.sidebar_width() as f64
    }

    pub(crate) fn on_wheel(&mut self, delta: ScrollDelta) {
        if self.wheel_targets_tabs()
            && self.ui.command_palette.is_none()
            && self.ui.goto_anything.is_none()
        {
            let pixels = match delta {
                ScrollDelta::Lines { x, y } => (x + y) * self.tab_width(),
                ScrollDelta::Pixels(position) => position.x as f32 + position.y as f32,
            };
            crate::chrome::scroll_tabs(self, pixels);
        } else if !crate::overlay_wheel::scroll_open_overlay(self, delta) {
            self.scroll(delta);
        }
    }

    pub(crate) fn scroll(&mut self, delta: ScrollDelta) {
        let delta = horizontal_when_shifted(self.modifiers.shift, delta);
        let (dx, dy) = scroll_offset_delta(delta, self.editor_view.metrics().line_height.max(1.0));
        let mouse_x = self.mouse.position.x as f32;
        let mouse_y = self.mouse.position.y as f32;
        if mouse_x < self.sidebar_width() && mouse_y >= self.tab_bar_height() {
            if let Some(sidebar) = self.ui.sidebar.as_mut() {
                sidebar.scroll_by(dy, self.scale_factor, self.viewport.height);
            }
            self.scheduler.invalidate_all();
            if let Some(gpu_window) = &self.gpu_window {
                gpu_window.window.request_redraw();
            }
            return;
        }
        if self.ui.split.is_active() {
            let primary_rect = [
                self.sidebar_width(),
                self.tab_bar_height(),
                self.viewport.width,
                self.viewport.height,
            ];
            if let Some(pane_idx) = crate::split::hit_test_pane(
                &self.ui.split,
                primary_rect,
                self.scale_factor,
                self.mouse.position.x as f32,
                self.mouse.position.y as f32,
            ) {
                if pane_idx == self.ui.split.active_pane {
                    self.apply_scroll_delta(dx, dy);
                    if pane_idx < self.ui.split.panes.len() {
                        self.ui.split.panes[pane_idx].viewport.scroll_x = self.viewport.scroll_x;
                        self.ui.split.panes[pane_idx].viewport.scroll_y = self.viewport.scroll_y;
                    }
                } else if pane_idx < self.ui.split.panes.len() {
                    let pane = &mut self.ui.split.panes[pane_idx].viewport;
                    pane.scroll_x = (pane.scroll_x - dx).max(0.0);
                    pane.scroll_y = (pane.scroll_y - dy).max(0.0);
                    if pane_idx == 1 {
                        self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
                    }
                }
                self.scheduler.invalidate_all();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
                return;
            }
        }
        self.apply_scroll_delta(dx, dy);
        self.scheduler.invalidate_all();
        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.request_redraw();
        }
    }

    fn apply_scroll_delta(&mut self, dx: f32, dy: f32) {
        if self.editor_view.metrics().word_wrap {
            self.viewport.scroll_x = 0.0;
        } else {
            self.viewport.scroll_x = (self.viewport.scroll_x - dx).max(0.0);
        }
        self.viewport.scroll_y = (self.viewport.scroll_y - dy).max(0.0);
    }
}

fn horizontal_when_shifted(shift: bool, delta: ScrollDelta) -> ScrollDelta {
    if !shift {
        return delta;
    }
    match delta {
        ScrollDelta::Lines { x, y } if x.abs() < 0.01 => ScrollDelta::Lines { x: y, y: 0.0 },
        ScrollDelta::Pixels(position) if position.x.abs() < 0.5 => {
            ScrollDelta::Pixels(dpi::PhysicalPosition::new(position.y, 0.0))
        }
        other => other,
    }
}

/// Physical-pixel movement for one wheel event.
///
/// `scroll_x` / `scroll_y` share the window's physical pixel space. winit's
/// macOS backend already converts precise trackpad deltas, including the
/// momentum phase, into physical pixels, so those are applied as-is. A notch
/// is one editor line at the current zoom and display scale.
pub(crate) fn scroll_offset_delta(delta: ScrollDelta, line_height: f32) -> (f32, f32) {
    let line_height = if line_height.is_finite() && line_height > 0.0 {
        line_height
    } else {
        1.0
    };
    match delta {
        ScrollDelta::Lines { x, y } => (x * line_height, y * line_height),
        ScrollDelta::Pixels(p) => (p.x as f32, p.y as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::scroll_offset_delta;
    use crate::app::{App, LINE_HEIGHT};
    use dpi::PhysicalPosition;
    use lux_platform::ScrollDelta;

    fn app() -> App {
        App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    #[test]
    fn trackpad_pixels_stay_physical_while_lines_follow_scale() {
        let line = scroll_offset_delta(ScrollDelta::Lines { x: 0.0, y: -1.0 }, LINE_HEIGHT * 2.0);
        assert!((line.1 + LINE_HEIGHT * 2.0).abs() < f32::EPSILON);
        let pixels = scroll_offset_delta(
            ScrollDelta::Pixels(PhysicalPosition::new(4.0, -20.0)),
            LINE_HEIGHT * 2.0,
        );
        assert_eq!(pixels, (4.0, -20.0));
    }

    #[test]
    fn scroll_applies_trackpad_pixels_on_both_axes() {
        let mut app = app();
        app.scale_factor = 2.0;
        app.scroll(ScrollDelta::Pixels(PhysicalPosition::new(-30.0, -40.0)));
        assert_eq!(app.viewport.scroll_x, 30.0);
        assert_eq!(app.viewport.scroll_y, 40.0);
    }

    #[test]
    fn a_momentum_tail_keeps_each_pixel_delta() {
        let mut app = app();
        let tail = [12.0_f64, 7.0, 3.0, 1.0];
        for dy in tail {
            app.scroll(ScrollDelta::Pixels(PhysicalPosition::new(-2.0, -dy)));
        }
        assert_eq!(app.viewport.scroll_x, 8.0);
        assert_eq!(app.viewport.scroll_y, 23.0);
        assert_ne!(app.viewport.scroll_y % LINE_HEIGHT, 0.0);
    }

    #[test]
    fn a_wheel_before_any_pointer_move_scrolls_the_editor() {
        let mut app = app();
        assert!(!app.mouse.placed);
        assert_eq!(app.mouse.position, dpi::PhysicalPosition::new(0.0, 0.0));
        app.on_wheel(ScrollDelta::Lines { x: 0.0, y: -8.0 });
        assert!(
            app.viewport.scroll_y > 0.0,
            "scroll_y was {}",
            app.viewport.scroll_y
        );
    }

    #[test]
    fn a_wheel_on_the_tab_strip_stays_there() {
        let mut app = app();
        app.mouse.placed = true;
        app.mouse.position = dpi::PhysicalPosition::new(200.0, 4.0);
        assert!(app.wheel_targets_tabs());
    }

    #[test]
    fn the_wheel_over_the_sidebar_scrolls_the_tree() {
        let dir = std::env::temp_dir().join(format!("lux-side-scroll-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for index in 0..40 {
            std::fs::write(dir.join(format!("file-{index:02}.txt")), "x").unwrap();
        }
        let mut app = app();
        app.open_workspace(dir.clone());
        app.mouse.position = dpi::PhysicalPosition::new(12.0, 80.0);
        app.scroll(ScrollDelta::Pixels(PhysicalPosition::new(0.0, -40.0)));
        let scrolled = app.ui.sidebar.unwrap().scroll_y;
        assert!(scrolled > 0.0, "sidebar scroll_y was {scrolled}");
        assert_eq!(app.viewport.scroll_y, 0.0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shift_wheel_scrolls_sideways_and_wrap_locks_horizontal_scroll() {
        let temp = tempfile::tempdir().unwrap();
        let mut app = app();
        app.ui.settings = crate::settings::SettingsState::new(temp.path().join("settings.json"));
        app.buffer = lux_core::buffer::Buffer::from_bytes(&vec![b'a'; 500]).unwrap();
        app.modifiers.shift = true;
        app.scroll(ScrollDelta::Lines { x: 0.0, y: -4.0 });
        assert!(app.viewport.scroll_x > 0.0);
        assert_eq!(app.viewport.scroll_y, 0.0);
        app.toggle_word_wrap();
        assert!(app.editor_view.metrics().word_wrap);
        assert_eq!(app.viewport.scroll_x, 0.0);
        app.scroll(ScrollDelta::Lines { x: 0.0, y: -4.0 });
        assert_eq!(app.viewport.scroll_x, 0.0);
        app.modifiers.shift = false;
        app.scroll(ScrollDelta::Pixels(PhysicalPosition::new(-40.0, -8.0)));
        assert_eq!(app.viewport.scroll_x, 0.0);
        assert!(app.viewport.scroll_y > 0.0);
    }
}
