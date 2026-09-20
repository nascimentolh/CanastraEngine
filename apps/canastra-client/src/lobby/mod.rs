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
use std::collections::BTreeMap;

use canastra_protocol::game::{CharacterId, CharacterSummary, InWorld, Move, Sex};
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
    player: Option<CharacterId>,
    /// Everyone the player can see in the world, its own character among them, in a steady order.
    in_world: BTreeMap<CharacterId, Presence>,
}

/// A character in the world: how it looked when it appeared, which dresses it, and where it is now.
struct Presence {
    shown: InWorld,
    at: [f32; 3],
    /// Which way it faces, in Unreal rotation units.
    yaw: i32,
    walk: Option<Walk>,
}

/// A walk in the world, as the server granted it.
struct Walk {
    from: [f32; 3],
    to: [f32; 3],
    /// Map units a second.
    speed: f32,
    /// Which way the walk faces, in Unreal rotation units.
    yaw: i32,
    started: std::time::Instant,
}

/// Where each character in the world stands this moment, which of them the player steers, and how far that
/// one's body reaches above its feet, which is where the camera looks.
pub(crate) struct Steering {
    pub(crate) steps: Vec<Step>,
    pub(crate) player: usize,
    pub(crate) middle: f32,
}

/// Where one character stands now, which way it faces and whether it is on its way.
pub(crate) struct Step {
    pub(crate) at: [f32; 3],
    pub(crate) yaw: i32,
    pub(crate) moving: bool,
}

/// What stands behind a screen: a lobby map framed by one of its own scenes, or the world tile a character
/// stands in, seen from behind it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Backdrop {
    Scene(&'static str, &'static str),
    /// The tile, where the character stands in it, which way it faces, and how far its body reaches above its
    /// feet, which is where H5's camera looks.
    World {
        map: String,
        at: [f32; 3],
        heading: i32,
        middle: f32,
    },
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
            player: None,
            in_world: BTreeMap::new(),
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

    /// What a character's class is called, in the player's language.
    pub(crate) fn class_name(&self, character: &CharacterSummary) -> String {
        let data = self.data.as_ref().ok();
        let class = data.and_then(|data| data.classes.get(&character.class));
        class.and_then(|class| class.name.get(canastra_data::text::Locale::En)).unwrap_or("?").to_owned()
    }

    /// Asks the server to walk the character to `to`, a place the player clicked in the world.
    pub(crate) fn walk_to(&mut self, to: [f32; 3]) {
        if self.player.is_none() {
            return;
        }
        #[expect(clippy::cast_possible_truncation, reason = "places are whole map units")]
        let to = to.map(|unit| unit.round() as i32);
        if let Ok(network) = &self.network {
            network.send(Request::Move(to));
        }
    }

    /// Where everyone in the world stands this moment: each one walks along the walk it was granted, and
    /// stays where that walk left it. The player's own place follows it, so the map tile it stands in does too.
    pub(crate) fn steering(&mut self) -> Option<Steering> {
        let steered = self.player?;
        let mut steps = Vec::with_capacity(self.in_world.len());
        let mut player = 0;
        for (index, (character, presence)) in self.in_world.iter_mut().enumerate() {
            if *character == steered {
                player = index;
            }
            steps.push(presence.step());
        }
        let middle = self.data.as_ref().ok().zip(self.in_world.get(&steered));
        let middle = middle.and_then(|(data, own)| middle_of(data, &own.shown.character)).unwrap_or_default();
        Some(Steering { steps, player, middle })
    }

    /// Where the character the player steers stands now.
    fn stands_at(&self) -> Option<&Presence> {
        self.in_world.get(&self.player?)
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
                let class = self.class_name(&world.character);
                screen.set("world.character".into(), world.character.name.clone());
                screen.set("world.detail".into(), format!("{class} · Level {}", world.character.level));
                self.player = Some(world.character.id);
                self.in_world.clear();
                self.in_world.insert(world.character.id, Presence::new(world));
                screen.show(WORLD_SCREEN);
                String::new()
            }
            Reply::Appears(who) => {
                self.in_world.insert(who.character.id, Presence::new(who));
                String::new()
            }
            Reply::Vanishes(character) => {
                if Some(character) != self.player {
                    self.in_world.remove(&character);
                }
                String::new()
            }
            Reply::Moving { character, walk } => {
                if let Some(presence) = self.in_world.get_mut(&character) {
                    presence.walk = Some(walk_of(&walk));
                }
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
        match (markup, self.stands_at()) {
            (WORLD_SCREEN, Some(own)) => {
                let middle = self.data.as_ref().ok().and_then(|data| middle_of(data, &own.shown.character));
                Backdrop::World {
                    map: crate::scene::map_at(own.at),
                    at: own.at,
                    heading: own.yaw,
                    middle: middle.unwrap_or_default(),
                }
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
            Backdrop::World { .. } => Vec::new(),
        }
    }

    /// The characters to stand in the scene behind `markup`.
    pub(crate) fn figures(&self, markup: &str) -> Vec<Figure> {
        match (&self.data, markup) {
            (Ok(data), WORLD_SCREEN) => self
                .in_world
                .values()
                .filter_map(|presence| {
                    let shown = &presence.shown;
                    figures::world(data, &shown.character, place(shown.position), shown.heading)
                })
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

/// The walk the server granted, in the places the scene measures in, facing the way it goes.
fn walk_of(walk: &Move) -> Walk {
    #[expect(clippy::cast_precision_loss, reason = "map units are whole numbers well within a float")]
    let place = |unit: i32| unit as f32;
    let (from, to) = (walk.from.map(place), walk.to.map(place));
    #[expect(clippy::cast_possible_truncation, reason = "an angle is a fraction of a turn")]
    let yaw = ((to[1] - from[1]).atan2(to[0] - from[0]) / std::f32::consts::TAU * 65536.0) as i32;
    Walk { from, to, speed: walk.speed, yaw, started: std::time::Instant::now() }
}

fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    from.iter().zip(to).map(|(from, to)| (to - from) * (to - from)).sum::<f32>().sqrt()
}

/// How far a character's body reaches above its feet, from its class template: Unreal keeps a pawn's
/// location at the middle of its collision cylinder, and that is what the camera looks at.
fn middle_of(data: &GameData, character: &CharacterSummary) -> Option<f32> {
    let start = data.starting_class(character.class)?;
    let body = match character.sex {
        Sex::Female => start.template.body_female,
        Sex::Male => start.template.body_male,
    };
    #[expect(clippy::cast_possible_truncation, reason = "collision sizes are tens of units")]
    Some(body.height as f32)
}

/// A place in map units, as the scene measures places.
fn place(at: [i32; 3]) -> [f32; 3] {
    #[expect(clippy::cast_precision_loss, reason = "map units are whole numbers well within a float")]
    at.map(|unit| unit as f32)
}

impl Presence {
    /// A character as it was shown, standing where it was shown standing.
    fn new(shown: InWorld) -> Self {
        let (at, yaw) = (place(shown.position), shown.heading);
        Self { shown, at, yaw, walk: None }
    }

    /// Moves the character along its walk and says where it now stands.
    fn step(&mut self) -> Step {
        let Some(walk) = &self.walk else {
            return Step { at: self.at, yaw: self.yaw, moving: false };
        };
        let gone = walk.speed * walk.started.elapsed().as_secs_f32();
        let length = distance(walk.from, walk.to);
        let part = if length > 0.0 { (gone / length).min(1.0) } else { 1.0 };
        let mut at = walk.from;
        for (at, to) in at.iter_mut().zip(walk.to) {
            *at += (to - *at) * part;
        }
        let arrived = part >= 1.0;
        (self.at, self.yaw) = (at, walk.yaw);
        if arrived {
            self.walk = None;
        }
        Step { at, yaw: self.yaw, moving: !arrived }
    }
}

fn bind_servers(servers: &[ServerEntry], screen: &mut Screen) {
    screen.set("servers.len".into(), servers.len().to_string());
    for (index, server) in servers.iter().enumerate() {
        screen.set(format!("servers.{index}.name"), server.name.clone());
        screen.set(format!("servers.{index}.load"), format!("{} / {}", server.population, server.capacity));
    }
}
