//! The character select screen: a card per character, the selected one's details, and deleting after asking.

use canastra_data::text::Locale;
use canastra_protocol::game::CharacterSummary;

use super::{CHARACTERS_SCREEN, DELETE_SCREEN, Lobby};
use crate::network::Request;
use crate::screen::Screen;

impl Lobby {
    /// Runs a select screen action; false when the action is not one.
    pub(super) fn act_select(&mut self, verb: &str, index: Option<usize>, screen: &mut Screen) -> bool {
        match (verb, index) {
            ("select", Some(index)) if index < self.characters.len() => self.selected = index,
            ("start", _) => {
                if let Some(character) = self.characters.get(self.selected) {
                    self.request(Request::Enter(character.id), "Entering the world...", screen);
                }
            }
            ("delete", _) => {
                let Some(character) = self.characters.get(self.selected) else { return true };
                let question = format!("Delete {}? This cannot be undone.", character.name);
                screen.set("delete.question".into(), question);
                screen.status.clear();
                screen.show(DELETE_SCREEN);
            }
            ("delete.confirm", _) => {
                if let Some(character) = self.characters.get(self.selected) {
                    self.request(Request::Delete(character.id), "Deleting...", screen);
                }
            }
            ("delete.cancel", _) => {
                screen.status.clear();
                screen.show(CHARACTERS_SCREEN);
            }
            _ => return false,
        }
        self.bind_characters(screen);
        true
    }

    /// Selects the character named `name` on the select screen, as when its card is clicked.
    pub(crate) fn pick(&mut self, name: &str, screen: &mut Screen) {
        if screen.markup() == CHARACTERS_SCREEN
            && let Some(index) = self.characters.iter().position(|character| character.name == name)
        {
            self.selected = index;
            self.bind_characters(screen);
        }
    }

    /// Binds a card per character and the selected character's details.
    pub(super) fn bind_characters(&self, screen: &mut Screen) {
        let class = |character: &CharacterSummary| {
            let data = self.data.as_ref().ok();
            let class = data.and_then(|data| data.classes.get(&character.class));
            class.and_then(|class| class.name.get(Locale::En)).unwrap_or("?").to_owned()
        };
        screen.set("characters.len".into(), self.characters.len().to_string());
        for (index, character) in self.characters.iter().enumerate() {
            screen.set(format!("characters.{index}.name"), character.name.clone());
            screen.set(format!("characters.{index}.detail"), format!("Level {}", character.level));
            screen.set(format!("characters.{index}.selected"), (index == self.selected).to_string());
        }
        let [name, class, level] = match self.characters.get(self.selected) {
            Some(character) => [character.name.clone(), class(character), format!("Level {}", character.level)],
            None => ["No characters yet".into(), "Create one to begin".into(), String::new()],
        };
        screen.set("selected.name".into(), name);
        screen.set("selected.class".into(), class);
        screen.set("selected.level".into(), level);
    }
}
