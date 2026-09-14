//! Window surface and device: backend choice, configuration and frame acquisition.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub(crate) struct Gpu {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    // Dropped last: the surface must go before the window it draws into.
    pub(crate) window: Arc<Window>,
}

impl Gpu {
    pub(crate) fn open(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Result<Self, String> {
        let (surface, adapter) = adapter(event_loop, &window)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .map_err(|error| error.to_string())?;
        let capabilities = surface.get_capabilities(&adapter);
        let format =
            capabilities.formats.iter().copied().find(wgpu::TextureFormat::is_srgb).ok_or("no sRGB surface format")?;
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: capabilities.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let info = adapter.get_info();
        println!("graphics: {:?} on {}", info.backend, info.name);
        Ok(Self { surface, config, device, queue, window })
    }

    /// Size of the drawable area in physical pixels.
    pub(crate) fn size(&self) -> [u32; 2] {
        [self.config.width, self.config.height]
    }

    pub(crate) fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    /// The next frame to draw into, or `None` when this one must be skipped (a redraw is queued).
    pub(crate) fn frame(&self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => return Some(texture),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Suboptimal(_) => {
                self.surface.configure(&self.device, &self.config);
            }
            _ => {}
        }
        self.window.request_redraw();
        None
    }
}

/// The first backend in platform order that gives a surface and an adapter for it.
fn adapter(
    event_loop: &ActiveEventLoop,
    window: &Arc<Window>,
) -> Result<(wgpu::Surface<'static>, wgpu::Adapter), String> {
    let order = match std::env::var("CANASTRA_BACKEND").ok().as_deref() {
        Some("dx12") => vec![wgpu::Backends::DX12],
        Some("vulkan") => vec![wgpu::Backends::VULKAN],
        Some("metal") => vec![wgpu::Backends::METAL],
        Some(other) => return Err(format!("unknown CANASTRA_BACKEND `{other}`")),
        None if cfg!(windows) => vec![wgpu::Backends::DX12, wgpu::Backends::VULKAN],
        None if cfg!(target_os = "macos") => vec![wgpu::Backends::METAL],
        None => vec![wgpu::Backends::VULKAN],
    };
    for backends in order {
        let mut descriptor =
            wgpu::InstanceDescriptor::new_with_display_handle(Box::new(event_loop.owned_display_handle()));
        descriptor.backends = backends;
        let instance = wgpu::Instance::new(descriptor);
        let Ok(surface) = instance.create_surface(window.clone()) else { continue };
        let options = wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() };
        if let Ok(adapter) = pollster::block_on(instance.request_adapter(&options)) {
            return Ok((surface, adapter));
        }
    }
    Err("no supported graphics adapter".into())
}
