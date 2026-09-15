//! The character being created: the starting classes to pick from, and the choices made so far.

use canastra_data::GameData;
use canastra_data::class::{Archetype, Origin};
use canastra_data::id::ClassId;
use canastra_data::npc::{Race, Sex as LineSex};
use canastra_data::text::Locale;
use canastra_protocol::game::{Appearance, NewCharacter, Sex};

/// The races characters are created as, in H5's order, with their names.
pub(super) const RACES: [(Race, &str); 6] = [
    (Race::Human, "Human"),
    (Race::Elf, "Elf"),
    (Race::DarkElf, "Dark Elf"),
    (Race::Orc, "Orc"),
    (Race::Dwarf, "Dwarf"),
    (Race::Kamael, "Kamael"),
];

/// A class characters start as, as the creation screen lists it.
pub(super) struct Choice {
    pub(super) id: ClassId,
    pub(super) name: String,
    /// The only sex the class allows, if any.
    pub(super) sex: Option<Sex>,
    pub(super) race: Race,
    pub(super) archetype: Archetype,
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
            Some(Choice {
                id: class.id,
                name: class.name.get(Locale::En).unwrap_or("?").to_owned(),
                sex,
                race: start.race,
                archetype: start.archetype,
            })
        })
        .collect()
}

/// A part of the look the steppers cycle through.
#[derive(Debug, Clone, Copy)]
pub(super) enum Look {
    HairStyle,
    HairColor,
    Face,
}

pub(super) struct Draft {
    pub(super) race: Race,
    /// Index into the choices.
    pub(super) class: Option<usize>,
    pub(super) sex: Option<Sex>,
    pub(super) appearance: Appearance,
}

impl Default for Draft {
    fn default() -> Self {
        Self { race: Race::Human, class: None, sex: None, appearance: Appearance::default() }
    }
}

impl Draft {
    /// The chosen race's classes, as indices into `choices`.
    pub(super) fn classes(&self, choices: &[Choice]) -> Vec<usize> {
        choices.iter().enumerate().filter(|(_, choice)| choice.race == self.race).map(|(index, _)| index).collect()
    }

    pub(super) fn choice<'a>(&self, choices: &'a [Choice]) -> Option<&'a Choice> {
        choices.get(self.class?)
    }

    /// Picks `race`, with no class or sex chosen yet, as H5 shows a race before its classes.
    pub(super) fn pick_race(&mut self, race: Race) {
        (self.race, self.class, self.sex) = (race, None, None);
    }

    /// Picks the class at `index`; a class tied to one sex picks that sex too.
    pub(super) fn pick_class(&mut self, choices: &[Choice], index: usize) {
        if let Some(choice) = choices.get(index) {
            (self.class, self.race) = (Some(index), choice.race);
            if let Some(sex) = choice.sex.or(self.sex) {
                self.pick_sex(choices, sex);
            }
        }
    }

    /// Picks `sex` unless the chosen class allows only the other, keeping the look within what it offers.
    pub(super) fn pick_sex(&mut self, choices: &[Choice], sex: Sex) {
        let sex = self.choice(choices).and_then(|choice| choice.sex).unwrap_or(sex);
        self.sex = Some(sex);
        let offered = Appearance::choices(self.race, sex);
        self.appearance.hair_style = self.appearance.hair_style.min(offered.hair_style.saturating_sub(1));
    }

    /// Moves `look` to its next choice, or its previous one, wrapping around what the sex offers.
    pub(super) fn step(&mut self, look: Look, forwards: bool) {
        let offered = Appearance::choices(self.race, self.sex.unwrap_or(Sex::Male));
        let (value, count) = match look {
            Look::HairStyle => (&mut self.appearance.hair_style, offered.hair_style),
            Look::HairColor => (&mut self.appearance.hair_color, offered.hair_color),
            Look::Face => (&mut self.appearance.face, offered.face),
        };
        let count = count.max(1);
        *value = if forwards { (*value + 1) % count } else { (*value + count - 1) % count };
    }

    /// The character to request, once a class and sex are chosen.
    pub(super) fn character(&self, choices: &[Choice], name: &str) -> Option<NewCharacter> {
        let (choice, sex) = (self.choice(choices)?, self.sex?);
        Some(NewCharacter { name: name.to_owned(), class: choice.id, sex, appearance: self.appearance })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn races_classes_sexes_and_looks_stay_consistent() {
        let choice =
            |id, race, sex| Choice { id: ClassId(id), name: String::new(), sex, race, archetype: Archetype::Fighter };
        let choices =
            [choice(0, Race::Human, None), choice(10, Race::Human, None), choice(124, Race::Kamael, Some(Sex::Female))];
        let mut draft = Draft::default();
        draft.pick_race(Race::Human);
        assert_eq!((draft.classes(&choices), draft.class, draft.sex), (vec![0, 1], None, None));
        draft.pick_class(&choices, 0);
        assert_eq!(draft.sex, None, "a class open to both sexes waits for one");
        assert!(draft.character(&choices, "Ana").is_none());

        draft.pick_sex(&choices, Sex::Female);
        draft.step(Look::HairStyle, false);
        assert_eq!(draft.appearance.hair_style, 6, "female styles wrap back to the seventh");
        draft.pick_sex(&choices, Sex::Male);
        assert_eq!(draft.appearance.hair_style, 4, "a male keeps only his five styles");

        draft.pick_race(Race::Kamael);
        draft.pick_class(&choices, 2);
        assert_eq!(draft.sex, Some(Sex::Female), "a class tied to one sex picks it");
        draft.appearance.hair_color = 0;
        draft.step(Look::HairColor, false);
        assert_eq!(draft.appearance.hair_color, 2, "Kamael hair comes in three colors");
        draft.pick_sex(&choices, Sex::Male);
        let new = draft.character(&choices, "Ana").unwrap();
        assert_eq!((new.class, new.sex), (ClassId(124), Sex::Female));
    }
}
