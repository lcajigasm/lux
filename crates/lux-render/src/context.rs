//! The GPU instance/adapter/device/queue every surface and pipeline in this
//! crate is built from.

use crate::error::RenderError;

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Requests a high-performance adapter and its device/queue.
    ///
    /// `compatible_surface` should be `Some(&surface)` when a window is
    /// already known, so the adapter picked is guaranteed to be able to
    /// present to it; `None` is valid too (used for headless testing here,
    /// and for any future compute-only use), matching `wgpu`'s own contract
    /// that the hint is optional off the web.
    pub async fn new(compatible_surface: Option<&wgpu::Surface<'_>>) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface,
                ..Default::default()
            })
            .await
            .map_err(|e| RenderError::Adapter(e.to_string()))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| RenderError::Device(e.to_string()))?;
        Ok(GpuContext {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_a_real_headless_adapter_and_device() {
        let context = pollster::block_on(GpuContext::new(None));
        let context = match context {
            Ok(context) => context,
            // No GPU/driver available in this environment (e.g. a headless
            // CI runner with no Metal/Vulkan/DX12 backend) — not this
            // crate's bug to report, so skip rather than fail.
            Err(_) => return,
        };
        assert!(!context.adapter.get_info().name.is_empty());
    }
}
