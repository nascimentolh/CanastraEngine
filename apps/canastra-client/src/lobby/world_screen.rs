//! The world's HUD: what the character the player steers has left to fight with, and what it has its eye on.

use canastra_protocol::game::{CharacterId, Pool};

use super::{HEARD, Lobby};
use crate::network::Request;
use crate::screen::Screen;

impl Lobby {
    /// Shows the player's own pools, each one as full as it is and with the points behind it.
    pub(super) fn bind_vitals(&self, screen: &mut Screen) {
        let vitals = self.stands_at().map(|own| own.shown.vitals).unwrap_or_default();
        for (pool, name) in [(vitals.cp, "cp"), (vitals.hp, "hp"), (vitals.mp, "mp")] {
            bind_pool(screen, &format!("world.{name}"), pool);
        }
    }

    /// Takes the character named `name` as what the player has its eye on. A name nobody in view carries
    /// leaves the target as it was, as a click that misses does in H5.
    // ponytail: the target is the client's own; tell the server once anything is aimed at one.
    pub(crate) fn aim_at(&mut self, name: &str, screen: &mut Screen) {
        let found = self.in_world.iter().find(|(_, presence)| presence.shown.character.name == name);
        if let Some(character) = found.map(|(character, _)| *character) {
            self.target = Some(character);
            self.bind_target(screen);
        }
    }

    /// Forgets what the player had its eye on.
    pub(super) fn drop_target(&mut self, screen: &mut Screen) {
        self.target = None;
        self.bind_target(screen);
    }

    /// Clears the target when it is `character`, which has left the player's view.
    pub(super) fn target_vanishes(&mut self, character: CharacterId, screen: &mut Screen) {
        if self.target == Some(character) {
            self.drop_target(screen);
        }
    }

    /// Shows the target's name, what it is and the life it has left; the window stays away without one.
    pub(super) fn bind_target(&self, screen: &mut Screen) {
        let aimed = self.target.and_then(|character| self.in_world.get(&character));
        let Some(presence) = aimed else {
            screen.set("target.shown".into(), String::new());
            return;
        };
        let character = &presence.shown.character;
        screen.set("target.shown".into(), "true".into());
        screen.set("target.name".into(), character.name.clone());
        screen.set("target.detail".into(), format!("{} · Level {}", self.class_name(character), character.level));
        bind_pool(screen, "target.hp", presence.shown.vitals.hp);
    }
}

impl Lobby {
    /// Says out loud what the player typed into the chat line, and leaves the line empty.
    pub(super) fn say(&mut self, screen: &mut Screen) {
        let text = screen.value(CHAT_LINE).trim().to_owned();
        if text.is_empty() {
            return;
        }
        screen.set(CHAT_LINE.into(), String::new());
        if let Ok(network) = &self.network {
            network.send(Request::Say(text));
        }
    }

    /// Takes a line someone said into the chat log, keeping the last `HEARD` of them.
    pub(super) fn heard(&mut self, from: &str, text: &str, screen: &mut Screen) {
        self.chat.push(format!("{from}: {text}"));
        if self.chat.len() > HEARD {
            self.chat.remove(0);
        }
        self.bind_chat(screen);
    }

    /// Shows the chat log, oldest line first.
    pub(super) fn bind_chat(&self, screen: &mut Screen) {
        screen.set("chat.len".into(), self.chat.len().to_string());
        for (index, line) in self.chat.iter().enumerate() {
            screen.set(format!("chat.{index}.text"), line.clone());
        }
    }
}

/// The data key the chat line is typed into.
const CHAT_LINE: &str = "chat.line";

/// Shows one pool under `key`: how full it is, which is how wide its bar is drawn, and the points behind it.
fn bind_pool(screen: &mut Screen, key: &str, pool: Pool) {
    screen.set(key.to_owned(), format!("{}%", pool.percent()));
    screen.set(format!("{key}-points"), format!("{} / {}", pool.now, pool.most));
}
