//! The creation screen: its combos and steppers bound to the draft, and the actions that change it.

use canastra_data::class::Origin;
use canastra_protocol::game::{Appearance, Sex};

use super::creation::{Draft, Look, RACES};
use super::{CHARACTERS_SCREEN, CREATE_SCREEN, Lobby};
use crate::network::Request;
use crate::screen::Screen;

const GENDERS: [(Sex, &str); 2] = [(Sex::Male, "Male"), (Sex::Female, "Female")];

impl Lobby {
    /// Runs a creation screen action; false when the action is not one.
    pub(super) fn act_creation(&mut self, verb: &str, index: Option<usize>, screen: &mut Screen) -> bool {
        let choices = &self.choices;
        if matches!(verb, "race" | "class" | "gender") {
            self.zoomed = false;
        }
        match (verb, index) {
            ("create", _) => return self.open_creation(screen),
            ("zoom", _) => self.zoomed = !self.zoomed && self.draft.sex.is_some() && self.draft.class.is_some(),
            ("race", Some(index)) => {
                if let Some(&(race, _)) = RACES.get(index) {
                    self.draft.pick_race(race);
                }
            }
            ("class", Some(index)) => {
                if let Some(&class) = self.draft.classes(choices).get(index) {
                    self.draft.pick_class(choices, class);
                }
            }
            ("gender", Some(index)) => {
                if let Some(&(sex, _)) = GENDERS.get(index) {
                    self.draft.pick_sex(choices, sex);
                }
            }
            ("hair.prev" | "hair.next", _) => self.draft.step(Look::HairStyle, verb.ends_with("next")),
            ("color.prev" | "color.next", _) => self.draft.step(Look::HairColor, verb.ends_with("next")),
            ("face.prev" | "face.next", _) => self.draft.step(Look::Face, verb.ends_with("next")),
            ("confirm", _) => match self.draft.character(choices, screen.value("create.name")) {
                Some(new) if !new.name.is_empty() => self.request(Request::Create(new), "Creating...", screen),
                _ => screen.status = "Enter a name and pick a class.".into(),
            },
            ("cancel", _) => {
                screen.status.clear();
                screen.show(CHARACTERS_SCREEN);
            }
            _ => return false,
        }
        self.bind_creation(screen);
        true
    }

    fn open_creation(&mut self, screen: &mut Screen) -> bool {
        if let Err(error) = &self.data {
            screen.status = format!("Cannot create characters: {error}");
            return true;
        }
        self.draft = Draft::default();
        screen.set("create.name".into(), String::new());
        self.bind_creation(screen);
        screen.status.clear();
        screen.show(CREATE_SCREEN);
        true
    }

    /// Binds the draft and the lists its combos open.
    fn bind_creation(&self, screen: &mut Screen) {
        let classes = self.draft.classes(&self.choices).into_iter().filter_map(|index| self.choices.get(index));
        bind_list(screen, "races", RACES.iter().map(|(_, name)| *name));
        bind_list(screen, "classes", classes.map(|choice| choice.name.as_str()));
        bind_list(screen, "genders", GENDERS.iter().map(|(_, name)| *name));

        let draft = &self.draft;
        let choice = draft.choice(&self.choices);
        let race = RACES.iter().find(|(race, _)| *race == draft.race).map_or("", |(_, name)| name);
        let gender =
            GENDERS.iter().find(|(sex, _)| Some(*sex) == draft.sex).map_or("Choose a gender", |(_, name)| name);
        let Appearance { hair_style, hair_color, face } = draft.appearance;
        for (key, value) in [
            ("create.race", race.to_owned()),
            ("create.class", choice.map_or_else(|| "Choose a class".to_owned(), |choice| choice.name.clone())),
            ("create.gender", gender.to_owned()),
            ("create.hair", letter(hair_style)),
            ("create.color", letter(hair_color)),
            ("create.face", letter(face)),
            ("create.zoom", if self.zoomed { "−" } else { "+" }.to_owned()),
        ] {
            screen.set(key.into(), value);
        }

        let start = self.data.as_ref().ok().zip(choice).and_then(|(data, choice)| {
            match &data.classes.get(&choice.id)?.origin {
                Origin::Starting(start) => Some(start.template.attributes),
                Origin::Advanced { .. } => None,
            }
        });
        let [body, mind] = start.map_or_else(Default::default, |a| {
            [
                format!("STR {}   DEX {}   CON {}", a.str, a.dex, a.con),
                format!("INT {}   WIT {}   MEN {}", a.int, a.wit, a.men),
            ]
        });
        screen.set("create.body".into(), body);
        screen.set("create.mind".into(), mind);
    }
}

/// Binds `names` as the data list `list`.
fn bind_list<'a>(screen: &mut Screen, list: &str, names: impl Iterator<Item = &'a str>) {
    let len =
        names.enumerate().map(|(index, name)| screen.set(format!("{list}.{index}.name"), name.to_owned())).count();
    screen.set(format!("{list}.len"), len.to_string());
}

/// A look choice as the creation screen names it: Type A, Type B and so on.
fn letter(choice: u8) -> String {
    format!("Type {}", char::from(b'A'.saturating_add(choice)))
}
