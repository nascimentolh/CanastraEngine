//! Window lifecycle and input routing between the screen, the GPU and the renderer.

use std::path::PathBuf;
use std::sync::Arc;

use canastra_protocol::login::{AuthFailure, ServerEntry, TicketRefusal};
use l2_catalog::Catalog;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::gpu::Gpu;
use crate::network::{LoginAddress, Network, Reply, Request};
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::screen::Screen;

/// The map behind the login screen and the scene that places its camera.
const LOGIN_MAP: &str = "lobby01.unr";
const LOGIN_CAMERA: &str = "Logon_Warp";

const LOGIN_SCREEN: &str = "login.ui";
const SERVERS_SCREEN: &str = "servers.ui";

pub(crate) struct App {
    client_root: PathBuf,
    ui_folder: PathBuf,
    proxy: EventLoopProxy<Reply>,
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
    /// The login server connection, or why it could not start.
    network: Result<Network, String>,
    /// The game servers the login server listed.
    servers: Vec<ServerEntry>,
    gpu: Gpu,
}

impl App {
    pub(crate) fn new(client_root: PathBuf, ui_folder: PathBuf, proxy: EventLoopProxy<Reply>) -> Self {
        Self { client_root, ui_folder, proxy, running: None, error: None }
    }

    fn start(&self, event_loop: &ActiveEventLoop) -> Result<Running, String> {
        let screen = Screen::load(self.ui_folder.clone(), LOGIN_SCREEN)?;
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
        let network = LoginAddress::from_env().and_then(|address| Network::start(address, self.proxy.clone()));
        Ok(Running { screen, modifiers: ModifiersState::empty(), scene, renderer, network, servers: Vec::new(), gpu })
    }
}

impl Running {
    fn redraw(&mut self) {
        let scale = self.gpu.scale();
        let [width, height] = self.gpu.size();
        self.screen.layout([width as f32 / scale, height as f32 / scale], self.renderer.fonts());
        let Some(frame) = self.gpu.frame() else { return };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let scene = self.scene.as_mut().map(|scene| scene.draw(&self.gpu, &frame.texture));
        match self.renderer.render(&self.gpu, &view, &self.screen.frame, scene.is_none()) {
            Ok(ui) => {
                self.gpu.queue.submit(scene.into_iter().chain([ui]));
                self.gpu.queue.present(frame);
                // The scene's materials move every frame; the UI only while a transition runs.
                if self.scene.is_some() || self.screen.frame.animating {
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
        match action {
            "exit" => event_loop.exit(),
            "login" => {
                let (account, password) = (self.screen.value("login.account"), self.screen.value("login.password"));
                if account.is_empty() || password.is_empty() {
                    self.screen.status = "Enter your account and password.".into();
                } else {
                    let request = Request::Login { account: account.to_owned(), password: password.to_owned() };
                    self.request(request, "Connecting...");
                }
            }
            "back" => {
                self.screen.status.clear();
                self.screen.show(LOGIN_SCREEN);
            }
            _ => match action.strip_prefix("server:").and_then(|index| index.parse::<usize>().ok()) {
                Some(index) => {
                    if let Some(server) = self.servers.get(index) {
                        self.request(Request::Ticket(server.id), "Joining...");
                    }
                }
                None => eprintln!("unknown action `{action}`"),
            },
        }
        self.gpu.window.request_redraw();
    }

    /// Sends `request` to the login server, showing `status` while it runs.
    fn request(&mut self, request: Request, status: &str) {
        match &self.network {
            Ok(network) => {
                network.send(request);
                self.screen.status = status.into();
            }
            Err(error) => self.screen.status = error.clone(),
        }
    }

    fn reply(&mut self, reply: Reply) {
        self.screen.status = match reply {
            Reply::Failed(error) => format!("Cannot reach the login server: {error}"),
            Reply::UpdateRequired => "This client is out of date. Please update.".into(),
            Reply::AuthFailed(AuthFailure::WrongCredentials) => "Wrong account or password.".into(),
            Reply::AuthFailed(AuthFailure::Banned) => "This account is banned.".into(),
            Reply::AuthFailed(AuthFailure::TooManyAttempts) => "Too many attempts. Try again later.".into(),
            Reply::Servers(servers) => {
                self.screen.set("servers.len".into(), servers.len().to_string());
                for (index, server) in servers.iter().enumerate() {
                    self.screen.set(format!("servers.{index}.name"), server.name.clone());
                    self.screen
                        .set(format!("servers.{index}.load"), format!("{} / {}", server.population, server.capacity));
                }
                let status = if servers.is_empty() { "No servers are online." } else { "" };
                self.servers = servers;
                self.screen.show(SERVERS_SCREEN);
                status.into()
            }
            Reply::Ticket(id, _ticket) => {
                let name =
                    self.servers.iter().find(|server| server.id == id).map_or("?", |server| server.name.as_str());
                // ponytail: the ticket is not used until the game server accepts players.
                format!("Ticket received for {name}.")
            }
            Reply::TicketRefused(TicketRefusal::Offline) => "That server is offline.".into(),
            Reply::TicketRefused(TicketRefusal::Full) => "That server is full.".into(),
        };
        self.gpu.window.request_redraw();
    }
}

impl ApplicationHandler<Reply> for App {
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

    fn user_event(&mut self, _: &ActiveEventLoop, reply: Reply) {
        if let Some(running) = &mut self.running {
            running.reply(reply);
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
