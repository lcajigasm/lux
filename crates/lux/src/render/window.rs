use dpi::{LogicalSize, PhysicalSize};
use lux_platform::{PlatformContext, PlatformWindow, ScrollDelta, WindowAttributes};
use lux_render::{GpuContext, QuadPipeline, Surface, TextPipeline};
use lux_ui::Theme;

use crate::app::{ATLAS_SIDE, App, GpuWindow, LINE_HEIGHT};
use crate::render::paint::color;

impl App {
    pub(crate) fn create_gpu_window(&mut self, ctx: &dyn PlatformContext) {
        let window = ctx
            .create_window(WindowAttributes {
                title: "Lux".to_string(),
                inner_size: Some(LogicalSize::new(1000.0, 700.0)),
                ..Default::default()
            })
            .expect("failed to create the main window");
        window.set_ime_allowed(true);
        let window: &'static dyn PlatformWindow = Box::leak(window);

        let size = window.inner_size();
        self.set_display_scale(window.scale_factor());
        let gpu = pollster::block_on(GpuContext::new(None))
            .expect("a real GPU adapter must be available to run the Lux desktop app");
        let surface = Surface::new(&gpu.instance, &gpu.adapter, &gpu.device, window, size)
            .expect("failed to create the window's wgpu surface");
        let format = surface.config().format;
        let quads = QuadPipeline::new(&gpu.device, format);
        let text = TextPipeline::new(&gpu.device, format, ATLAS_SIDE);
        quads.set_viewport_size(&gpu.queue, size.width, size.height);
        text.set_globals(
            &gpu.queue,
            size.width,
            size.height,
            color(Theme::LUX_DARK.background),
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
            color(Theme::LUX_DARK.background),
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

    pub(crate) fn scroll(&mut self, delta: ScrollDelta) {
        let dy = match delta {
            ScrollDelta::Lines { y, .. } => y * LINE_HEIGHT * self.scale_factor,
            ScrollDelta::Pixels(p) => p.y as f32,
        };
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
                    self.viewport.scroll_y = (self.viewport.scroll_y - dy).max(0.0);
                    if pane_idx < self.ui.split.panes.len() {
                        self.ui.split.panes[pane_idx].viewport.scroll_y = self.viewport.scroll_y;
                    }
                } else if pane_idx < self.ui.split.panes.len() {
                    let cur = self.ui.split.panes[pane_idx].viewport.scroll_y;
                    self.ui.split.panes[pane_idx].viewport.scroll_y = (cur - dy).max(0.0);
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
        self.viewport.scroll_y = (self.viewport.scroll_y - dy).max(0.0);
        self.scheduler.invalidate_all();
        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.request_redraw();
        }
    }
}
