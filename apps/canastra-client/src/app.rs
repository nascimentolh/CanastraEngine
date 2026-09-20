//! Window lifecycle and input routing between the screen, the GPU and the renderer.

use std::path::PathBuf;
use std::sync::Arc;

use canastra_ui::{Draw, Rect, Rgba, TextAlign, TextStyle};
use l2_catalog::Catalog;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::gpu::Gpu;
use crate::lobby::{Backdrop, LOGIN_SCREEN, Lobby};
use crate::network::{LoginAddress, Network, Reply};
use crate::renderer::Renderer;
use crate::scene::{Figure, Scene, View};
use crate::screen::Screen;

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
    /// What `scene` was loaded to stand behind the screen.
    backdrop: Backdrop,
    /// The characters standing in `scene`.
    figures: Vec<Figure>,
    /// The camera view `scene` shows or is flying to, as the lobby names it.
    view: String,
    /// The action of the control the pointer holds down.
    held: Option<String>,
    client_root: PathBuf,
    renderer: Renderer,
    lobby: Lobby,
    gpu: Gpu,
}

impl App {
    pub(crate) fn new(client_root: PathBuf, ui_folder: PathBuf, proxy: EventLoopProxy<Reply>) -> Self {
        Self { client_root, ui_folder, proxy, running: None, error: None }
    }

    fn start(&self, event_loop: &ActiveEventLoop) -> Result<Running, String> {
        let mut screen = Screen::load(self.ui_folder.clone(), LOGIN_SCREEN)?;
        let attributes = Window::default_attributes()
            .with_title("Canastra")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = Arc::new(event_loop.create_window(attributes).map_err(|error| error.to_string())?);
        let gpu = Gpu::open(event_loop, window)?;
        let mut renderer = Renderer::new(&gpu.device, &gpu.queue, gpu.config.format, Catalog::open(&self.client_root));
        // ponytail: fonts load once at start; F5 reloads markup and CSS but not new font files.
        let faces = renderer.fonts().load_folder(&self.ui_folder.join("fonts"));
        println!("fonts: {faces} faces from {}", self.ui_folder.join("fonts").display());
        let network = LoginAddress::from_env().and_then(|address| Network::start(address, self.proxy.clone()));
        let mut lobby =
            Lobby::new(network, game_data().inspect_err(|error| eprintln!("game data: {error}")), &self.client_root);
        lobby.bind_options(&mut screen);
        let (backend, device) = gpu.description();
        screen.set("app.backend".into(), backend);
        screen.set("app.device".into(), device);
        // Development shortcut for automated captures: logs in at start without typing.
        if let Some((account, password)) =
            std::env::var("CANASTRA_AUTOLOGIN").ok().as_deref().and_then(|value| value.split_once(':'))
        {
            screen.set("login.account".into(), account.to_owned());
            screen.set("login.password".into(), password.to_owned());
            lobby.act("login", &mut screen);
        }
        let backdrop = lobby.backdrop(LOGIN_SCREEN);
        let mut scene = load_scene(&gpu, &self.client_root, &backdrop);
        lobby.play_ambient(scene.as_mut().map(Scene::ambient_sounds).unwrap_or_default());
        let client_root = self.client_root.clone();
        Ok(Running {
            screen,
            modifiers: ModifiersState::empty(),
            scene,
            backdrop,
            figures: Vec::new(),
            view: String::new(),
            held: None,
            client_root,
            renderer,
            lobby,
            gpu,
        })
    }
}

impl Running {
    fn redraw(&mut self) {
        self.lobby.tick_audio();
        let scale = self.gpu.scale();
        let [width, height] = self.gpu.size();
        self.screen.layout([width as f32 / scale, height as f32 / scale], self.renderer.fonts());
        if let Some(scene) = &self.scene {
            // Text draws over every shape, so names that would show through a panel are left out.
            let panels: Vec<Rect> = self
                .screen
                .frame
                .draws
                .iter()
                .filter_map(|draw| match draw {
                    Draw::Rect { rect, fill: Some(_), .. } => Some(*rect),
                    _ => None,
                })
                .collect();
            let tags: Vec<Draw> = scene
                .labels([width, height])
                .into_iter()
                .map(|(label, [x, y], _)| name_tag(label, [x / scale, y / scale]))
                .filter(|tag| !panels.iter().any(|panel| panel.intersects(tag.rect())))
                .collect();
            // Names stand in the scene, under the UI's overlays.
            let from = self.screen.frame.overlay_from;
            self.screen.frame.overlay_from += tags.len();
            self.screen.frame.draws.splice(from..from, tags);
        }
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
        if action == "exit" {
            event_loop.exit();
        } else if !self.lobby.act(action, &mut self.screen) {
            eprintln!("unknown action `{action}`");
        }
        self.follow_screen();
        self.gpu.window.request_redraw();
    }

    /// Selects the character under the pointer, when it points at one in the scene.
    fn pick(&mut self) {
        let scale = self.gpu.scale();
        let (x, y) = self.screen.pointer_at();
        let Some(name) = self.scene.as_ref().and_then(|scene| scene.label_at(self.gpu.size(), [x * scale, y * scale]))
        else {
            return;
        };
        self.lobby.pick(name, &mut self.screen);
        self.follow_screen();
    }

    /// Loads the scene the shown screen stands in and stands the lobby's characters in it, when they changed.
    // ponytail: loads block the window for a moment; load in the background if it shows.
    fn follow_screen(&mut self) {
        let backdrop = self.lobby.backdrop(self.screen.markup());
        if backdrop != self.backdrop {
            // Another scene of the same map only moves the camera, when it lands in a zone lit the same way.
            let warped = match (&self.backdrop, &backdrop) {
                (Backdrop::Scene(loaded, _), Backdrop::Scene(map, camera)) if loaded == map => {
                    self.scene.as_mut().is_some_and(|scene| scene.warp(camera))
                }
                _ => false,
            };
            if !warped {
                self.scene = load_scene(&self.gpu, &self.client_root, &backdrop);
            }
            self.backdrop = backdrop;
            let sounds = self.scene.as_mut().map(Scene::ambient_sounds).unwrap_or_default();
            self.lobby.play_ambient(sounds);
            self.lobby.follow_music(self.screen.markup(), &self.client_root);
            self.figures.clear();
            self.view.clear();
        }
        let view = self.lobby.view(self.screen.markup());
        if view != self.view {
            if let Some(scene) = &mut self.scene {
                scene.travel(&self.lobby.routes(self.screen.markup(), &self.view, &view));
            }
            self.view = view;
        }
        if let Some(scene) = &mut self.scene {
            scene.turn(self.lobby.turning(self.screen.markup()));
        }
        let figures = self.lobby.figures(self.screen.markup());
        if figures != self.figures
            && let Some(scene) = &mut self.scene
        {
            scene.place(&self.gpu, &figures);
            self.figures = figures;
        }
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
                let on_ui = running.screen.hovered();
                if let Some(action) = running.screen.click() {
                    running.run(event_loop, &action);
                    running.held = Some(action);
                } else if !on_ui {
                    running.pick();
                }
                running.gpu.window.request_redraw();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                // Held controls, such as turning the character, stop when let go.
                if running.held.take().is_some_and(|action| action.starts_with("turn.")) {
                    running.run(event_loop, "turn.stop");
                }
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
            running.lobby.reply(reply, &mut running.screen);
            running.follow_screen();
            running.gpu.window.request_redraw();
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

/// A name drawn centered above the point `at`, in logical pixels, about as wide as its letters.
fn name_tag(label: &str, [x, y]: [f32; 2]) -> Draw {
    const LETTER: f32 = 8.0;
    const HEIGHT: f32 = 18.0;
    let width = label.chars().count() as f32 * LETTER;
    Draw::Text {
        rect: Rect { x: x - width / 2.0, y: y - HEIGHT, width, height: HEIGHT },
        text: label.to_owned(),
        color: Rgba([0xf2, 0xe6, 0xc8, 0xff]),
        style: TextStyle { family: None, size: 13.0, weight: 500, letter_spacing: 0.0, align: TextAlign::Center },
    }
}

/// The scene standing behind the screen, or `None` with the reason logged.
fn load_scene(gpu: &Gpu, client_root: &std::path::Path, backdrop: &Backdrop) -> Option<Scene> {
    let (map, view) = match backdrop {
        Backdrop::Scene(map, camera) => (*map, View::Scene(camera)),
        Backdrop::World(map, at) => (map.as_str(), View::Behind(*at)),
    };
    let started = std::time::Instant::now();
    Scene::load(gpu, client_root, map, view)
        .inspect(|_| println!("scene loaded in {:?}", started.elapsed()))
        .inspect_err(|error| eprintln!("scene: {error}"))
        .ok()
}

/// The game data at `CANASTRA_GAME_DATA`, by default `gamedata.cana` in the working folder.
// ponytail: read from a path until content packs deliver the data each server plays by.
fn game_data() -> Result<canastra_data::GameData, String> {
    let path = std::env::var("CANASTRA_GAME_DATA").unwrap_or_else(|_| "gamedata.cana".into());
    let bytes = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
    canastra_data::format::decode(&bytes).map_err(|error| format!("{path}: {error}"))
}
