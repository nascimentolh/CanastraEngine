//! Window lifecycle and input routing between the screen, the GPU and the renderer.

use std::path::PathBuf;
use std::sync::Arc;

use l2_catalog::Catalog;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::gpu::Gpu;
use crate::renderer::Renderer;
use crate::screen::Screen;

pub(crate) struct App {
    client_root: PathBuf,
    ui_folder: PathBuf,
    running: Option<Running>,
    /// Why the app stopped early, reported after the event loop ends.
    pub(crate) error: Option<String>,
}

struct Running {
    screen: Screen,
    renderer: Renderer,
    gpu: Gpu,
}

impl App {
    pub(crate) fn new(client_root: PathBuf, ui_folder: PathBuf) -> Self {
        Self { client_root, ui_folder, running: None, error: None }
    }

    fn start(&self, event_loop: &ActiveEventLoop) -> Result<Running, String> {
        let screen = Screen::load(self.ui_folder.clone())?;
        let attributes = Window::default_attributes()
            .with_title("Canastra")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attributes).map_err(|error| error.to_string())?);
        let gpu = Gpu::open(event_loop, window)?;
        let renderer = Renderer::new(&gpu.device, &gpu.queue, gpu.config.format, Catalog::open(&self.client_root));
        Ok(Running { screen, renderer, gpu })
    }
}

impl Running {
    fn redraw(&mut self) {
        let scale = self.gpu.scale();
        let [width, height] = self.gpu.size();
        self.screen.layout([width as f32 / scale, height as f32 / scale], self.renderer.fonts());
        let Some(frame) = self.gpu.frame() else { return };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        match self.renderer.render(&self.gpu.device, &self.gpu.queue, &view, self.gpu.size(), scale, &self.screen.frame)
        {
            Ok(commands) => {
                self.gpu.queue.submit([commands]);
                self.gpu.queue.present(frame);
            }
            Err(error) => eprintln!("render: {error}"),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match self.start(event_loop) {
            Ok(running) => self.running = Some(running),
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(running) = &mut self.running else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => running.gpu.resize(size.width, size.height),
            WindowEvent::RedrawRequested => running.redraw(),
            WindowEvent::CursorMoved { position, .. } => {
                let scale = f64::from(running.gpu.scale());
                if running.screen.pointer((position.x / scale) as f32, (position.y / scale) as f32) {
                    running.gpu.window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                match running.screen.click().map(str::to_owned).as_deref() {
                    Some("exit") => event_loop.exit(),
                    Some(action) => {
                        println!("action: {action}");
                        running.screen.status = format!("Action: {action}");
                        running.gpu.window.request_redraw();
                    }
                    None => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::F5) =>
            {
                running.screen.reload();
                running.gpu.window.request_redraw();
            }
            _ => {}
        }
    }
}
