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
use crate::scene::{Figure, Scene, SceneData, View};
use crate::screen::Screen;

/// Frames between reports of how long a frame takes.
const REPORT: u32 = 120;

/// What reaches the window loop from elsewhere: the network thread's replies and the scenes read off the
/// window's thread.
pub(crate) enum Event {
    Network(Reply),
    /// What a map was read as, and what it was read to stand behind; `None` when it could not be read.
    Read(Backdrop, Box<Option<SceneData>>),
}

pub(crate) struct App {
    client_root: PathBuf,
    ui_folder: PathBuf,
    proxy: EventLoopProxy<Event>,
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
    /// What a thread is reading now, so the same map is not read twice over.
    reading: Option<Backdrop>,
    /// How long each frame took since the last time it was reported.
    frames: Vec<std::time::Duration>,
    proxy: EventLoopProxy<Event>,
    client_root: PathBuf,
    renderer: Renderer,
    lobby: Lobby,
    gpu: Gpu,
}

impl App {
    pub(crate) fn new(client_root: PathBuf, ui_folder: PathBuf, proxy: EventLoopProxy<Event>) -> Self {
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
        // The login's scene is read here, before the window shows anything, so the player never sees it empty.
        let backdrop = lobby.backdrop(LOGIN_SCREEN);
        let mut scene =
            read_scene(&self.client_root, &backdrop, gpu.blocks).and_then(|data| build_scene(&gpu, &backdrop, data));
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
            reading: None,
            frames: Vec::new(),
            proxy: self.proxy.clone(),
            client_root,
            renderer,
            lobby,
            gpu,
        })
    }
}

impl Running {
    fn redraw(&mut self) {
        let started = std::time::Instant::now();
        self.lobby.tick_audio();
        self.steer();
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
                // Frames are timed and reported once a second, so a heavy scene says so in the log.
                self.frames.push(started.elapsed());
                let count = u32::try_from(self.frames.len()).unwrap_or(1);
                if count >= REPORT {
                    let mean = self.frames.iter().sum::<std::time::Duration>() / count;
                    println!("frame: {mean:?} over {count} frames");
                    self.frames.clear();
                }
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

    /// Answers a click in the scene: in the lobby it picks the character under the pointer, in the world it
    /// walks the character to the floor the pointer is on.
    fn pick(&mut self) {
        let scale = self.gpu.scale();
        let (x, y) = self.screen.pointer_at();
        let (size, at) = (self.gpu.size(), [x * scale, y * scale]);
        if let Some(name) = self.scene.as_ref().and_then(|scene| scene.label_at(size, at)) {
            self.lobby.pick(name, &mut self.screen);
            self.follow_screen();
            return;
        }
        if let (Backdrop::World { .. }, Some(scene)) = (&self.backdrop, &self.scene)
            && let Some(floor) = scene.ground_at(size, at)
        {
            self.lobby.walk_to(floor);
        }
    }

    /// Follows the shown screen: reads the scene it stands in when that changed, and stands the lobby's
    /// characters in the scene already shown.
    fn follow_screen(&mut self) {
        let backdrop = self.lobby.backdrop(self.screen.markup());
        if backdrop != self.backdrop && self.reading.as_ref() != Some(&backdrop) {
            // Another scene of the same map only moves the camera, when it lands in a zone lit the same way.
            let warped = match (&self.backdrop, &backdrop) {
                (Backdrop::Scene(loaded, _), Backdrop::Scene(map, camera)) if loaded == map => {
                    self.scene.as_mut().is_some_and(|scene| scene.warp(camera))
                }
                _ => false,
            };
            if warped {
                self.settle(backdrop);
            } else {
                self.read_in_background(backdrop);
            }
        }
        // While a map is being read, the scene still up is the one it replaces: nothing new belongs in it.
        if self.reading.is_some() {
            return;
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

    /// Moves the character the player steers to where it stands this frame, and reads the next map tile when it
    /// walked into one.
    fn steer(&mut self) {
        if !matches!(self.backdrop, Backdrop::World { .. }) {
            return;
        }
        let Some(steering) = self.lobby.steering() else { return };
        if let Some(scene) = &mut self.scene {
            let steps: Vec<([f32; 3], i32, bool)> =
                steering.steps.iter().map(|step| (step.at, step.yaw, step.moving)).collect();
            scene.steer(&steps, steering.player, steering.middle);
        }
        if self.lobby.backdrop(self.screen.markup()) != self.backdrop {
            self.follow_screen();
        }
    }

    /// Reads a map on a thread of its own, so the window keeps drawing while it is read; the scene it becomes
    /// arrives as an [`Event::Read`].
    fn read_in_background(&mut self, backdrop: Backdrop) {
        self.reading = Some(backdrop.clone());
        let (client_root, proxy) = (self.client_root.clone(), self.proxy.clone());
        let blocks = self.gpu.blocks;
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let read = read_scene(&client_root, &backdrop, blocks);
            println!("scene read in {:?}", started.elapsed());
            let _ = proxy.send_event(Event::Read(backdrop, Box::new(read)));
        });
    }

    /// Takes the scene read for `backdrop` as the one standing behind the screen.
    fn read(&mut self, backdrop: &Backdrop, data: Option<SceneData>) {
        if self.reading.as_ref() == Some(backdrop) {
            self.reading = None;
        }
        // A scene read for a screen the player has left since is dropped; the one they are on is on its way.
        if self.lobby.backdrop(self.screen.markup()) != *backdrop {
            return;
        }
        self.scene = data.and_then(|data| build_scene(&self.gpu, backdrop, data));
        self.settle(backdrop.clone());
        self.follow_screen();
    }

    /// Takes `backdrop` as the one shown, and lets what follows the scene follow it.
    fn settle(&mut self, backdrop: Backdrop) {
        self.backdrop = backdrop;
        let sounds = self.scene.as_mut().map(Scene::ambient_sounds).unwrap_or_default();
        self.lobby.play_ambient(sounds);
        self.lobby.follow_music(self.screen.markup(), &self.client_root);
        self.figures.clear();
        self.view.clear();
    }
}

impl ApplicationHandler<Event> for App {
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

    fn user_event(&mut self, _: &ActiveEventLoop, event: Event) {
        let Some(running) = &mut self.running else { return };
        match event {
            Event::Network(reply) => {
                running.lobby.reply(reply, &mut running.screen);
                running.follow_screen();
            }
            Event::Read(backdrop, data) => running.read(&backdrop, *data),
        }
        running.gpu.window.request_redraw();
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

/// What stands behind `backdrop`, read from the client, or `None` with the reason logged.
fn read_scene(client_root: &std::path::Path, backdrop: &Backdrop, blocks: bool) -> Option<SceneData> {
    let (map, view) = match backdrop {
        Backdrop::Scene(map, camera) => (*map, View::Scene(camera)),
        Backdrop::World { map, at, heading, middle } => (map.as_str(), View::Behind(*at, *heading, *middle)),
    };
    Scene::read(client_root, map, view, blocks).inspect_err(|error| eprintln!("scene: {error}")).ok()
}

/// The scene of what was read, on the GPU, or `None` with the reason logged.
fn build_scene(gpu: &Gpu, backdrop: &Backdrop, data: SceneData) -> Option<Scene> {
    let map = match backdrop {
        Backdrop::Scene(map, _) => *map,
        Backdrop::World { map, .. } => map.as_str(),
    };
    Scene::build(gpu, map, data).inspect_err(|error| eprintln!("scene: {error}")).ok()
}

/// The game data at `CANASTRA_GAME_DATA`, by default `gamedata.cana` in the working folder.
// ponytail: read from a path until content packs deliver the data each server plays by.
fn game_data() -> Result<canastra_data::GameData, String> {
    let path = std::env::var("CANASTRA_GAME_DATA").unwrap_or_else(|_| "gamedata.cana".into());
    let bytes = std::fs::read(&path).map_err(|error| format!("{path}: {error}"))?;
    canastra_data::format::decode(&bytes).map_err(|error| format!("{path}: {error}"))
}
