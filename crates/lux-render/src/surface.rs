//! Surface creation, format/present-mode selection, and resize.

use dpi::PhysicalSize;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::RenderError;

/// A `wgpu` surface bound to a window for as long as `'window` lives.
///
/// Built via the `unsafe` raw-handle path (`SurfaceTargetUnsafe`) rather
/// than `wgpu::Instance::create_surface`, which demands owning the window —
/// `lux-render` doesn't own windows (`lux-platform` or a future mobile
/// backend does), so this borrows one instead. The safety invariant that
/// obligation carries ("the window outlives the surface") is upheld by the
/// borrow checker: `window` is a `&'window` reference, so `Surface<'window>`
/// literally cannot outlive it.
pub struct Surface<'window> {
    surface: wgpu::Surface<'window>,
    config: wgpu::SurfaceConfiguration,
}

impl<'window> Surface<'window> {
    pub fn new(
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        window: &'window (impl HasWindowHandle + HasDisplayHandle + ?Sized),
        size: PhysicalSize<u32>,
    ) -> Result<Self, RenderError> {
        // Extracted through the object-safe accessor methods rather than
        // `SurfaceTargetUnsafe::from_display_and_window` (which takes `&impl
        // Trait` arguments and so demands `Sized`): callers here are
        // expected to hand over `&dyn PlatformWindow`-shaped trait objects,
        // and the raw handles themselves (plain, `Copy`, `Sized` enums) are
        // all `wgpu` actually needs.
        let raw_window_handle = window
            .window_handle()
            .map_err(|e| RenderError::Surface(e.to_string()))?
            .as_raw();
        let raw_display_handle = window
            .display_handle()
            .map_err(|e| RenderError::Surface(e.to_string()))?
            .as_raw();
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(raw_display_handle),
            raw_window_handle,
        };
        // SAFETY: `window` outlives this call, and the resulting
        // `Surface<'window>` is bounded by the same lifetime, so it cannot
        // outlive `window` either.
        // SAFETY: same invariant as above — `target` was just derived from
        // `window`, which is guaranteed to outlive the returned `Surface`.
        let surface = unsafe { instance.create_surface_unsafe(target) }
            .map_err(|e| RenderError::Surface(e.to_string()))?;

        let capabilities = surface.get_capabilities(adapter);
        let format =
            pick_format(&capabilities.formats).ok_or(RenderError::NoCompatibleSurfaceFormat)?;
        let present_mode = pick_present_mode(&capabilities.present_modes);
        let size = effective_size(size);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width,
            height: size.height,
            desired_maximum_frame_latency: 2,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        surface.configure(device, &config);

        Ok(Surface { surface, config })
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// Reconfigure an outdated swapchain even when its size is unchanged.
    pub fn reconfigure(&self, device: &wgpu::Device) {
        self.surface.configure(device, &self.config);
    }

    /// Reconfigures the surface for a new size. A no-op when the size
    /// hasn't actually changed, so a stream of identical resize events
    /// (winit can send these) doesn't stall the GPU waiting to reconfigure
    /// a swapchain that's already correct.
    pub fn resize(&mut self, device: &wgpu::Device, size: PhysicalSize<u32>) {
        let size = effective_size(size);
        if size.width == self.config.width && size.height == self.config.height {
            return;
        }
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(device, &self.config);
    }

    /// Acquires the next frame to render into. Callers must handle
    /// `Outdated` with `reconfigure`; `Lost` requires a new Surface.
    pub fn acquire_frame(&self) -> wgpu::CurrentSurfaceTexture {
        self.surface.get_current_texture()
    }
}

/// `wgpu` panics if a surface is configured with a zero dimension (e.g. a
/// minimized window reporting `0x0`); clamp instead of propagating that
/// panic into every caller.
fn effective_size(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    PhysicalSize::new(size.width.max(1), size.height.max(1))
}

/// The lowest-latency mode the surface actually offers.
///
/// `Fifo` waits for the next vertical blank. On a 60 Hz display that wait
/// is one 16.7 ms refresh, so a key-to-pixel sample cannot land under 8 ms.
/// Metal's list is `Fifo` then `Immediate` and has no mailbox mode, so the
/// non-blocking choice there is `Immediate`. Vulkan prefers `Mailbox`.
fn pick_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    for mode in [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Immediate] {
        if modes.contains(&mode) {
            return mode;
        }
    }
    modes.first().copied().unwrap_or(wgpu::PresentMode::Fifo)
}

/// Prefers an sRGB-capable format so alpha-blended text and UI composite
/// correctly against the framebuffer; falls back to whatever the surface
/// reports first otherwise.
fn pick_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .or_else(|| formats.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_sized_window_clamps_to_one_pixel() {
        assert_eq!(
            effective_size(PhysicalSize::new(0, 0)),
            PhysicalSize::new(1, 1)
        );
        assert_eq!(
            effective_size(PhysicalSize::new(0, 600)),
            PhysicalSize::new(1, 600)
        );
        assert_eq!(
            effective_size(PhysicalSize::new(800, 0)),
            PhysicalSize::new(800, 1)
        );
    }

    #[test]
    fn normal_size_passes_through_unchanged() {
        assert_eq!(
            effective_size(PhysicalSize::new(1600, 1200)),
            PhysicalSize::new(1600, 1200)
        );
    }

    #[test]
    fn prefers_srgb_format_when_available() {
        let formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ];
        assert_eq!(
            pick_format(&formats),
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        );
    }

    #[test]
    fn srgb_preference_is_order_independent() {
        let formats = [
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        assert_eq!(
            pick_format(&formats),
            Some(wgpu::TextureFormat::Rgba8UnormSrgb)
        );
    }

    #[test]
    fn falls_back_to_first_format_when_no_srgb_variant_exists() {
        let formats = [
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba8Unorm,
        ];
        assert_eq!(
            pick_format(&formats),
            Some(wgpu::TextureFormat::Rgba16Float)
        );
    }

    #[test]
    fn empty_format_list_yields_none() {
        assert_eq!(pick_format(&[]), None);
    }

    #[test]
    fn present_mode_prefers_mailbox_then_immediate() {
        assert_eq!(
            pick_present_mode(&[
                wgpu::PresentMode::Fifo,
                wgpu::PresentMode::Mailbox,
                wgpu::PresentMode::Immediate,
            ]),
            wgpu::PresentMode::Mailbox
        );
        assert_eq!(
            pick_present_mode(&[wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate]),
            wgpu::PresentMode::Immediate
        );
    }

    #[test]
    fn present_mode_keeps_fifo_when_it_is_the_only_mode() {
        assert_eq!(
            pick_present_mode(&[wgpu::PresentMode::Fifo]),
            wgpu::PresentMode::Fifo
        );
        assert_eq!(pick_present_mode(&[]), wgpu::PresentMode::Fifo);
    }
}
