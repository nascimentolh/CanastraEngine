//! The way into the game: logging in, picking a server, and listing, creating and deleting characters. It
//! turns screen actions into network requests and network replies into screens and bound data.

mod attributes;
mod creation;
mod creation_screen;
mod figures;
mod messages;
mod select_screen;
mod views;

use canastra_data::GameData;
use canastra_data::npc::Race;
use canastra_protocol::game::{CharacterSummary, InWorld};
use canastra_protocol::login::ServerEntry;

use crate::audio::{Audio, Kind};
use crate::network::{Network, Reply, Request};
use crate::scene::{Figure, Route};
use crate::screen::Screen;
use creation::{Choice, Draft};

pub(crate) const LOGIN_SCREEN: &str = "login.ui";
/// The maps behind the lobby and the scenes that place the camera for each screen.
const MAP: &str = "lobby01.unr";
const LOGIN_CAMERA: &str = "Logon_Warp";
const SELECT_CAMERA: &str = "Char_Select_Warp";
const CREATION_MAP: &str = "Lobby02.unr";
const SERVERS_SCREEN: &str = "servers.ui";
const CHARACTERS_SCREEN: &str = "characters.ui";
const DELETE_SCREEN: &str = "delete.ui";
const CREATE_SCREEN: &str = "create.ui";
const WORLD_SCREEN: &str = "world.ui";
/// The track the lobby plays, which `MusicInfo` lists first as `INTRO`.
const LOBBY_MUSIC: &str = "intro";
/// The settings tabs the options window shows, the first one open when it opens.
const TABS: [&str; 2] = ["audio", "graphics"];

pub(crate) struct Lobby {
    /// The network thread, or why it could not start.
    network: Result<Network, String>,
    /// The game data this client plays by, or why it could not be read.
    data: Result<GameData, String>,
    servers: Vec<ServerEntry>,
    characters: Vec<CharacterSummary>,
    /// Index into the characters of the one standing in front.
    selected: usize,
    choices: Vec<Choice>,
    draft: Draft,
    /// Whether the creation camera closes in on the chosen character.
    zoomed: bool,
    /// Which way the player turns the chosen character: -1, 0 or 1.
    turning: i8,
    /// What the lobby plays, when a sound device opened.
    audio: Option<Audio>,
    /// The settings tab open in the options window, or `None` while it is closed.
    options: Option<&'static str>,
    /// The character the player is in the world with, once it entered.
    world: Option<InWorld>,
}

/// What stands behind a screen: a lobby map framed by one of its own scenes, or the world tile a character
/// stands in, seen from behind it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Backdrop {
    Scene(&'static str, &'static str),
    World(String, [f32; 3]),
}

impl Lobby {
    /// The lobby, with its music playing from the client under `client_root`.
    pub(crate) fn new(
        network: Result<Network, String>,
        data: Result<GameData, String>,
        client_root: &std::path::Path,
    ) -> Self {
        let choices = data.as_ref().map(creation::choices).unwrap_or_default();
        let mut audio = Audio::open();
        match &mut audio {
            Some(audio) => audio.play_music(client_root, LOBBY_MUSIC),
            None => eprintln!("sound: no device opened; the lobby plays in silence"),
        }
        Self {
            network,
            data,
            servers: Vec::new(),
            characters: Vec::new(),
            selected: 0,
            choices,
            draft: Draft::default(),
            zoomed: false,
            turning: 0,
            audio,
            options: None,
            world: None,
        }
    }

    /// Shows the volumes and which options tab is open, as the options window reads them.
    pub(crate) fn bind_options(&self, screen: &mut Screen) {
        for (kind, key) in [(Kind::Music, "audio.music"), (Kind::Effects, "audio.effects")] {
            let volume = self.audio.as_ref().map_or(0, |audio| audio.volume(kind));
            screen.set(key.into(), format!("{volume}%"));
        }
        let shown = |open: bool| if open { "true".to_owned() } else { String::new() };
        let muted = self.audio.as_ref().is_some_and(Audio::muted);
        screen.set("audio.muted".into(), shown(muted));
        screen.set("audio.mute-label".into(), if muted { "Off".into() } else { "On".into() });
        screen.set("options.open".into(), shown(self.options.is_some()));
        screen.set("options.closed".into(), shown(self.options.is_none()));
        for tab in TABS {
            screen.set(format!("options.{tab}"), shown(self.options == Some(tab)));
        }
    }

    /// The lobby's theme plays on the login screen; the halls beyond it are carried by their own sounds.
    pub(crate) fn follow_music(&mut self, markup: &str, client_root: &std::path::Path) {
        let Some(audio) = &mut self.audio else { return };
        if markup == LOGIN_SCREEN {
            audio.play_music(client_root, LOBBY_MUSIC);
        } else {
            audio.stop_music();
        }
    }

    /// Plays the sounds heard where the camera stands, in place of the ones playing now.
    pub(crate) fn play_ambient(&mut self, sounds: Vec<crate::audio::Clip>) {
        println!("sound: {} ambient loops", sounds.len());
        if let Some(audio) = &mut self.audio {
            audio.play_ambient(sounds);
        }
    }

    /// Keeps the music going; the lobby's track starts again each time it plays out.
    pub(crate) fn tick_audio(&mut self) {
        if let Some(audio) = &mut self.audio {
            audio.tick();
        }
    }

    /// Runs a screen action; false when the action is not the lobby's.
    pub(crate) fn act(&mut self, action: &str, screen: &mut Screen) -> bool {
        let (verb, index) =
            action.split_once(':').map_or((action, None), |(verb, index)| (verb, index.parse::<usize>().ok()));
        match (verb, index) {
            ("login", _) => {
                let (account, password) = (screen.value("login.account"), screen.value("login.password"));
                if account.is_empty() || password.is_empty() {
                    screen.status = "Enter your account and password.".into();
                } else {
                    let request = Request::Login { account: account.to_owned(), password: password.to_owned() };
                    self.request(request, "Connecting...", screen);
                }
            }
            ("options.open", _) => {
                self.options = Some(TABS.first().copied().unwrap_or_default());
                self.bind_options(screen);
            }
            ("options.close", _) => {
                self.options = None;
                self.bind_options(screen);
            }
            (tab, _) if TABS.iter().any(|name| verb == format!("options.{name}")) => {
                self.options = TABS.iter().copied().find(|name| tab.ends_with(name));
                self.bind_options(screen);
            }
            ("mute", _) => {
                if let Some(audio) = &mut self.audio {
                    let muted = audio.muted();
                    audio.mute(!muted);
                }
                self.bind_options(screen);
            }
            ("music.up" | "music.down" | "effects.up" | "effects.down", _) => {
                let kind = if verb.starts_with("music") { Kind::Music } else { Kind::Effects };
                if let Some(audio) = &mut self.audio {
                    audio.turn(kind, verb.ends_with("up"));
                }
                self.bind_options(screen);
            }
            ("back", _) => {
                screen.status.clear();
                screen.show(LOGIN_SCREEN);
            }
            ("server", Some(index)) => {
                if let Some(server) = self.servers.get(index) {
                    self.request(Request::Join(server.clone()), "Joining...", screen);
                }
            }
            _ => return self.act_select(verb, index, screen) || self.act_creation(verb, index, screen),
        }
        true
    }

    pub(crate) fn reply(&mut self, reply: Reply, screen: &mut Screen) {
        screen.status = match reply {
            Reply::Failed(error) => format!("Connection lost: {error}"),
            Reply::UpdateRequired => "This client is out of date. Please update.".into(),
            Reply::AuthFailed(failure) => messages::auth(failure).into(),
            Reply::TicketRefused(refusal) => messages::ticket(refusal).into(),
            Reply::GameRefused(refusal) => messages::admission(refusal).into(),
            Reply::Servers(servers) => {
                bind_servers(&servers, screen);
                self.servers = servers;
                screen.show(SERVERS_SCREEN);
                if self.servers.is_empty() { "No servers are online.".into() } else { String::new() }
            }
            Reply::Entered(world) => {
                let name = world.character.name.clone();
                self.world = Some(world);
                screen.set("world.character".into(), name);
                screen.show(WORLD_SCREEN);
                String::new()
            }
            Reply::Characters { list, failure: Some(failure) } => {
                self.characters = list;
                messages::creation(failure).into()
            }
            Reply::Characters { list, failure: None } => {
                self.characters = list;
                self.selected = self.selected.min(self.characters.len().saturating_sub(1));
                self.bind_characters(screen);
                screen.show(CHARACTERS_SCREEN);
                if self.characters.is_empty() { "Create your first character.".into() } else { String::new() }
            }
        };
    }

    /// What stands behind `markup`.
    pub(crate) fn backdrop(&self, markup: &str) -> Backdrop {
        match (markup, &self.world) {
            (WORLD_SCREEN, Some(world)) => {
                let at = stands_at(world);
                Backdrop::World(crate::scene::map_at(at), at)
            }
            (CHARACTERS_SCREEN | DELETE_SCREEN, _) => Backdrop::Scene(MAP, SELECT_CAMERA),
            // Lobby02 has one scene per race, all but Orc's named after it.
            (CREATE_SCREEN, _) => Backdrop::Scene(
                CREATION_MAP,
                match self.draft.race {
                    Race::Elf => "Elf",
                    Race::DarkElf => "DarkElf",
                    Race::Orc => "orc",
                    Race::Dwarf => "Dwarf",
                    Race::Kamael => "Kamael",
                    _ => "Human",
                },
            ),
            _ => Backdrop::Scene(MAP, LOGIN_CAMERA),
        }
    }

    /// The camera view of the scene behind `markup`, as `views` names them; empty for a scene's own camera.
    pub(crate) fn view(&self, markup: &str) -> String {
        match markup {
            CREATE_SCREEN => {
                let archetype = self.draft.choice(&self.choices).map(|choice| choice.archetype);
                views::view(archetype, self.draft.sex, self.zoomed)
            }
            _ => String::new(),
        }
    }

    /// How fast the characters the player may turn behind `markup` turn, in rotation units a second.
    pub(crate) fn turning(&self, markup: &str) -> f32 {
        // ponytail: a quarter turn a second, by eye; H5's pace is unmeasured.
        const QUARTER_TURN: f32 = 16384.0;
        if markup == CREATE_SCREEN { f32::from(self.turning) * QUARTER_TURN } else { 0.0 }
    }

    /// The camera routes from view `from` to view `to` of the scene behind `markup`; the world has none.
    pub(crate) fn routes(&self, markup: &str, from: &str, to: &str) -> Vec<Route> {
        match self.backdrop(markup) {
            Backdrop::Scene(_, camera) => views::routes(camera, from, to),
            Backdrop::World(..) => Vec::new(),
        }
    }

    /// The characters to stand in the scene behind `markup`.
    pub(crate) fn figures(&self, markup: &str) -> Vec<Figure> {
        match (&self.data, markup) {
            (Ok(data), WORLD_SCREEN) => self
                .world
                .as_ref()
                .and_then(|world| figures::world(data, &world.character, stands_at(world)))
                .into_iter()
                .collect(),
            (Ok(data), CHARACTERS_SCREEN | DELETE_SCREEN) => figures::select(data, &self.characters, self.selected),
            (Ok(data), CREATE_SCREEN) => {
                let chosen = self.draft.choice(&self.choices).map(|choice| choice.archetype).zip(self.draft.sex);
                figures::creation(data, self.draft.race, chosen, self.draft.appearance)
            }
            _ => Vec::new(),
        }
    }

    fn request(&self, request: Request, status: &str, screen: &mut Screen) {
        match &self.network {
            Ok(network) => {
                network.send(request);
                screen.status = status.into();
            }
            Err(error) => screen.status.clone_from(error),
        }
    }
}

/// Where a character stands, as the scene measures places.
fn stands_at(world: &InWorld) -> [f32; 3] {
    #[expect(clippy::cast_precision_loss, reason = "map units are whole numbers well within a float")]
    world.position.map(|unit| unit as f32)
}

fn bind_servers(servers: &[ServerEntry], screen: &mut Screen) {
    screen.set("servers.len".into(), servers.len().to_string());
    for (index, server) in servers.iter().enumerate() {
        screen.set(format!("servers.{index}.name"), server.name.clone());
        screen.set(format!("servers.{index}.load"), format!("{} / {}", server.population, server.capacity));
    }
}
