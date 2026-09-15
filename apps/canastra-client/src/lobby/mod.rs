//! The way into the game: logging in, picking a server, and listing, creating and deleting characters. It
//! turns screen actions into network requests and network replies into screens and bound data.

mod creation;
mod creation_screen;
mod figures;
mod messages;
mod views;

use canastra_data::GameData;
use canastra_data::npc::Race;
use canastra_data::text::Locale;
use canastra_protocol::game::CharacterSummary;
use canastra_protocol::login::ServerEntry;

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
const CREATE_SCREEN: &str = "create.ui";

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
}

impl Lobby {
    pub(crate) fn new(network: Result<Network, String>, data: Result<GameData, String>) -> Self {
        let choices = data.as_ref().map(creation::choices).unwrap_or_default();
        Self {
            network,
            data,
            servers: Vec::new(),
            characters: Vec::new(),
            selected: 0,
            choices,
            draft: Draft::default(),
            zoomed: false,
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
            ("back", _) => {
                screen.status.clear();
                screen.show(LOGIN_SCREEN);
            }
            ("server", Some(index)) => {
                if let Some(server) = self.servers.get(index) {
                    self.request(Request::Join(server.clone()), "Joining...", screen);
                }
            }
            ("select", Some(index)) if index < self.characters.len() => self.selected = index,
            ("delete", Some(index)) => {
                if let Some(character) = self.characters.get(index) {
                    self.request(Request::Delete(character.id), "Deleting...", screen);
                }
            }
            _ => return self.act_creation(verb, index, screen),
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
            Reply::Characters { list, failure: Some(failure) } => {
                self.characters = list;
                messages::creation(failure).into()
            }
            Reply::Characters { list, failure: None } => {
                self.bind_characters(&list, screen);
                self.characters = list;
                self.selected = self.selected.min(self.characters.len().saturating_sub(1));
                screen.show(CHARACTERS_SCREEN);
                if self.characters.is_empty() { "Create your first character.".into() } else { String::new() }
            }
        };
    }

    /// The map and camera scene shown behind `markup`.
    pub(crate) fn backdrop(&self, markup: &str) -> (&'static str, &'static str) {
        match markup {
            CHARACTERS_SCREEN => (MAP, SELECT_CAMERA),
            // Lobby02 has one scene per race, all but Orc's named after it.
            CREATE_SCREEN => (
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
            _ => (MAP, LOGIN_CAMERA),
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

    /// The camera routes from view `from` to view `to` of the scene behind `markup`.
    pub(crate) fn routes(&self, markup: &str, from: &str, to: &str) -> Vec<Route> {
        views::routes(self.backdrop(markup).1, from, to)
    }

    /// The characters to stand in the scene behind `markup`.
    pub(crate) fn figures(&self, markup: &str) -> Vec<Figure> {
        match (&self.data, markup) {
            (Ok(data), CHARACTERS_SCREEN) => figures::select(data, &self.characters, self.selected),
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

    fn bind_characters(&self, list: &[CharacterSummary], screen: &mut Screen) {
        screen.set("characters.len".into(), list.len().to_string());
        for (index, character) in list.iter().enumerate() {
            let class = self
                .data
                .as_ref()
                .ok()
                .and_then(|data| data.classes.get(&character.class))
                .and_then(|class| class.name.get(Locale::En))
                .unwrap_or("?");
            screen.set(format!("characters.{index}.name"), character.name.clone());
            screen.set(format!("characters.{index}.detail"), format!("{class}, level {}", character.level));
        }
    }
}

fn bind_servers(servers: &[ServerEntry], screen: &mut Screen) {
    screen.set("servers.len".into(), servers.len().to_string());
    for (index, server) in servers.iter().enumerate() {
        screen.set(format!("servers.{index}.name"), server.name.clone());
        screen.set(format!("servers.{index}.load"), format!("{} / {}", server.population, server.capacity));
    }
}
