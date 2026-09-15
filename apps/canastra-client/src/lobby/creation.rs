//! The character being created: the starting classes to pick from, and the choices made so far.

use canastra_data::GameData;
use canastra_data::class::Origin;
use canastra_data::id::ClassId;
use canastra_data::npc::Sex as LineSex;
use canastra_data::text::Locale;
use canastra_protocol::game::{Appearance, NewCharacter, Sex};

/// A class characters start as, as the creation screen lists it.
pub(super) struct Choice {
    pub(super) id: ClassId,
    pub(super) name: String,
    /// The only sex the class allows, if any.
    pub(super) sex: Option<Sex>,
}

/// The starting classes in `data`, by id.
pub(super) fn choices(data: &GameData) -> Vec<Choice> {
    data.classes
        .values()
        .filter_map(|class| {
            let Origin::Starting(start) = &class.origin else { return None };
            let sex = match start.sex {
                Some(LineSex::Male) => Some(Sex::Male),
                Some(LineSex::Female) => Some(Sex::Female),
                _ => None,
            };
            Some(Choice { id: class.id, name: class.name.get(Locale::En).unwrap_or("?").to_owned(), sex })
        })
        .collect()
}

#[derive(Default)]
pub(super) struct Draft {
    /// Index into the choices.
    pub(super) class: Option<usize>,
    pub(super) sex: Option<Sex>,
}

impl Draft {
    /// Picks the class at `index`; a class tied to one sex picks that sex too.
    pub(super) fn pick_class(&mut self, choices: &[Choice], index: usize) {
        if let Some(choice) = choices.get(index) {
            self.class = Some(index);
            self.sex = choice.sex.or(self.sex);
        }
    }

    /// Picks `sex` unless the chosen class allows only the other.
    pub(super) fn pick_sex(&mut self, choices: &[Choice], sex: Sex) {
        let fixed = self.class.and_then(|index| choices.get(index)).and_then(|choice| choice.sex);
        self.sex = Some(fixed.unwrap_or(sex));
    }

    /// What the screen shows about the choices so far.
    pub(super) fn summary(&self, choices: &[Choice]) -> String {
        let class = self.class.and_then(|index| choices.get(index)).map_or("No class", |choice| choice.name.as_str());
        let sex = match self.sex {
            Some(Sex::Male) => "Male",
            Some(Sex::Female) => "Female",
            None => "no sex chosen",
        };
        format!("{class}, {sex}")
    }

    /// The character to request, once a class and a sex are chosen.
    // ponytail: appearance stays at the first style, color and face until the creation screen shows the model.
    pub(super) fn character(&self, choices: &[Choice], name: &str) -> Option<NewCharacter> {
        let choice = choices.get(self.class?)?;
        Some(NewCharacter {
            name: name.to_owned(),
            class: choice.id,
            sex: self.sex?,
            appearance: Appearance::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_tied_to_one_sex_keeps_it() {
        let choices = [
            Choice { id: ClassId(0), name: "Human Fighter".into(), sex: None },
            Choice { id: ClassId(124), name: "Female Soldier".into(), sex: Some(Sex::Female) },
        ];
        let mut draft = Draft::default();
        assert!(draft.character(&choices, "Ana").is_none());
        draft.pick_class(&choices, 1);
        draft.pick_sex(&choices, Sex::Male);
        assert_eq!(draft.summary(&choices), "Female Soldier, Female");
        draft.pick_class(&choices, 0);
        draft.pick_sex(&choices, Sex::Male);
        assert_eq!(draft.character(&choices, "Ana").map(|new| (new.class, new.sex)), Some((ClassId(0), Sex::Male)));
    }
}
