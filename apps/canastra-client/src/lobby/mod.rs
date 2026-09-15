//! The way into the game: logging in, picking a server, and listing, creating and deleting characters. It
//! turns screen actions into network requests and network replies into screens and bound data.

mod creation;
mod messages;

use canastra_data::GameData;
use canastra_data::text::Locale;
use canastra_protocol::game::{CharacterSummary, Sex};
use canastra_protocol::login::ServerEntry;

use crate::network::{Network, Reply, Request};
use crate::screen::Screen;
use creation::{Choice, Draft};

pub(crate) const LOGIN_SCREEN: &str = "login.ui";
/// The map behind the lobby and the scene that places the camera for each screen.
const MAP: &str = "lobby01.unr";
const LOGIN_CAMERA: &str = "Logon_Warp";
const SELECT_CAMERA: &str = "Char_Select_Warp";
const SERVERS_SCREEN: &str = "servers.ui";
const CHARACTERS_SCREEN: &str = "characters.ui";
const CREATE_SCREEN: &str = "create.ui";

/// The map and camera scene shown behind `markup`.
// ponytail: creation stands in the select hall until its race scenes in Lobby02 are wired.
pub(crate) fn backdrop(markup: &str) -> (&'static str, &'static str) {
    match markup {
        CHARACTERS_SCREEN | CREATE_SCREEN => (MAP, SELECT_CAMERA),
        _ => (MAP, LOGIN_CAMERA),
    }
}

pub(crate) struct Lobby {
    /// The network thread, or why it could not start.
    network: Result<Network, String>,
    /// The game data this client plays by, or why it could not be read.
    data: Result<GameData, String>,
    servers: Vec<ServerEntry>,
    characters: Vec<CharacterSummary>,
    choices: Vec<Choice>,
    draft: Draft,
}

impl Lobby {
    pub(crate) fn new(network: Result<Network, String>, data: Result<GameData, String>) -> Self {
        let choices = data.as_ref().map(creation::choices).unwrap_or_default();
        Self { network, data, servers: Vec::new(), characters: Vec::new(), choices, draft: Draft::default() }
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
            ("delete", Some(index)) => {
                if let Some(character) = self.characters.get(index) {
                    self.request(Request::Delete(character.id), "Deleting...", screen);
                }
            }
            ("create", _) => self.open_creation(screen),
            ("class", Some(index)) => {
                self.draft.pick_class(&self.choices, index);
                screen.set("create.choice".into(), self.draft.summary(&self.choices));
            }
            ("male" | "female", _) => {
                self.draft.pick_sex(&self.choices, if verb == "male" { Sex::Male } else { Sex::Female });
                screen.set("create.choice".into(), self.draft.summary(&self.choices));
            }
            ("confirm", _) => match self.draft.character(&self.choices, screen.value("create.name")) {
                Some(new) if !new.name.is_empty() => self.request(Request::Create(new), "Creating...", screen),
                _ => screen.status = "Enter a name and pick a class and a sex.".into(),
            },
            ("cancel", _) => {
                screen.status.clear();
                screen.show(CHARACTERS_SCREEN);
            }
            _ => return false,
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
                screen.show(CHARACTERS_SCREEN);
                if self.characters.is_empty() { "Create your first character.".into() } else { String::new() }
            }
        };
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

    fn open_creation(&mut self, screen: &mut Screen) {
        if let Err(error) = &self.data {
            screen.status = format!("Cannot create characters: {error}");
            return;
        }
        self.draft = Draft::default();
        screen.set("classes.len".into(), self.choices.len().to_string());
        for (index, choice) in self.choices.iter().enumerate() {
            screen.set(format!("classes.{index}.name"), choice.name.clone());
        }
        screen.set("create.name".into(), String::new());
        screen.set("create.choice".into(), self.draft.summary(&self.choices));
        screen.status.clear();
        screen.show(CREATE_SCREEN);
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
