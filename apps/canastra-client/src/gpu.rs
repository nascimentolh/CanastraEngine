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
    /// The backend and the adapter drawing, as the options window shows them.
    description: (String, String),
    /// Whether this GPU reads the client's compressed textures as they are.
    pub(crate) blocks: bool,
}

impl Gpu {
    /// The backend and the adapter drawing, such as `Dx12` and `Intel(R) Iris(R) Xe Graphics`.
    pub(crate) fn description(&self) -> (String, String) {
        self.description.clone()
    }

    pub(crate) fn open(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Result<Self, String> {
        let (surface, adapter) = adapter(event_loop, &window)?;
        // Block compressed textures are what the client stores; a GPU that reads them saves both the work of
        // unpacking every texture and the memory of holding it unpacked.
        let blocks = adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let wanted = wgpu::DeviceDescriptor {
            required_features: if blocks { wgpu::Features::TEXTURE_COMPRESSION_BC } else { wgpu::Features::empty() },
            ..wgpu::DeviceDescriptor::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wanted)).map_err(|error| error.to_string())?;
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
            // The scene blends in gamma space like the original client, through a plain view.
            view_formats: vec![format.remove_srgb_suffix()],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let info = adapter.get_info();
        let description = (format!("{:?}", info.backend), info.name.clone());
        println!("graphics: {} on {}", description.0, description.1);
        Ok(Self { surface, config, device, queue, window, description, blocks })
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
