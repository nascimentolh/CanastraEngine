//! Window lifecycle and input routing between the screen, the GPU and the renderer.

use std::path::PathBuf;
use std::sync::Arc;

use l2_catalog::Catalog;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::gpu::Gpu;
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::screen::Screen;

/// The map behind the login screen and the scene that places its camera.
const LOGIN_MAP: &str = "lobby01.unr";
const LOGIN_CAMERA: &str = "Logon_Warp";

pub(crate) struct App {
    client_root: PathBuf,
    ui_folder: PathBuf,
    running: Option<Running>,
    /// Why the app stopped early, reported after the event loop ends.
    pub(crate) error: Option<String>,
}

struct Running {
    screen: Screen,
    modifiers: ModifiersState,
    /// `None` when the client's map could not be loaded; the UI then draws over black.
    scene: Option<Scene>,
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
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, gpu.config.format, Catalog::open(&self.client_root));
        // ponytail: fonts load once at start; F5 reloads markup and CSS but not new font files.
        let faces = renderer.fonts().load_folder(&self.ui_folder.join("fonts"));
        println!("fonts: {faces} faces from {}", self.ui_folder.join("fonts").display());
        let started = std::time::Instant::now();
        let scene = Scene::load(&gpu, &self.client_root, LOGIN_MAP, LOGIN_CAMERA)
            .inspect(|_| println!("scene loaded in {:?}", started.elapsed()))
            .inspect_err(|error| eprintln!("scene: {error}"))
            .ok();
        Ok(Running { screen, modifiers: ModifiersState::empty(), scene, renderer, gpu })
    }
}

impl Running {
    fn redraw(&mut self) {
        let scale = self.gpu.scale();
        let [width, height] = self.gpu.size();
        self.screen.layout([width as f32 / scale, height as f32 / scale], self.renderer.fonts());
        let Some(frame) = self.gpu.frame() else { return };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let scene = self.scene.as_mut().map(|scene| scene.draw(&self.gpu, &view));
        match self.renderer.render(&self.gpu, &view, &self.screen.frame, scene.is_none()) {
            Ok(ui) => {
                self.gpu.queue.submit(scene.into_iter().chain([ui]));
                self.gpu.queue.present(frame);
                if self.screen.frame.animating {
                    self.gpu.window.request_redraw();
                }
            }
            Err(error) => eprintln!("render: {error}"),
        }
    }

    fn key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        match &event.logical_key {
            Key::Named(NamedKey::F5) => self.screen.reload(),
            Key::Named(NamedKey::Tab) => self.screen.focus_next(self.modifiers.shift_key()),
            Key::Named(NamedKey::Backspace) => self.screen.backspace(),
            Key::Named(NamedKey::Enter) => {
                if let Some(action) = self.screen.submit() {
                    self.run(event_loop, &action);
                }
            }
            _ => {
                if let Some(text) = &event.text {
                    self.screen.type_text(text);
                }
            }
        }
        self.gpu.window.request_redraw();
    }

    fn run(&mut self, event_loop: &ActiveEventLoop, action: &str) {
        if action == "exit" {
            event_loop.exit();
            return;
        }
        println!("action: {action}");
        self.screen.status = format!("Action: {action}");
        self.gpu.window.request_redraw();
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
                if let Some(action) = running.screen.click() {
                    running.run(event_loop, &action);
                }
                running.gpu.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => running.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                running.key(event_loop, &event);
            }
            _ => {}
        }
    }

    fn new_events(&mut self, _: &ActiveEventLoop, cause: StartCause) {
        if let (StartCause::ResumeTimeReached { .. }, Some(running)) = (cause, &self.running) {
            running.gpu.window.request_redraw();
        }
    }

    /// Sleeps until the next event, or until the focused input's caret blinks.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let blink = self.running.as_ref().and_then(|running| running.screen.next_blink());
        event_loop.set_control_flow(blink.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
}
