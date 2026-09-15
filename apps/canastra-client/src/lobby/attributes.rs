//! The six base attributes as the creation screen shows them, each with a tip on what it does in High Five.

use canastra_data::class::Attributes;

use crate::screen::Screen;

/// Each attribute's key, as `create.<key>` binds its value and `create.<key>.tip` its tip, and the tip.
const TIPS: [(&str, &str); 6] = [
    ("str", "Strength\nThe raw might of the body. Raises P. Atk., the damage of weapons and physical skills."),
    ("dex", "Dexterity\nSpeed and precision. Raises Atk. Spd., Accuracy, Evasion, Critical Rate and running speed."),
    (
        "con",
        "Constitution\nThe endurance to stay standing. Raises Max HP and CP and their recovery, how much can be \
         carried, and resistance to stun, poison and bleeding.",
    ),
    ("int", "Intelligence\nMastery of the arcane. Raises M. Atk., the power of attack and healing spells."),
    ("wit", "Wit\nQuickness of mind. Raises Casting Spd. and the chance of magic critical hits."),
    (
        "men",
        "Mentality\nThe strength of the spirit. Raises M. Def., Max MP and its recovery, and helps hold a spell \
         together when struck.",
    ),
];

/// Binds each attribute of `attributes`, or blanks when no class is chosen, and every tip.
pub(super) fn bind(screen: &mut Screen, attributes: Option<Attributes>) {
    let values = attributes.map(|a| [a.str, a.dex, a.con, a.int, a.wit, a.men]);
    for (index, (key, tip)) in TIPS.iter().enumerate() {
        let value = values.and_then(|values| values.get(index).copied()).map(|value| value.to_string());
        screen.set(format!("create.{key}"), value.unwrap_or_default());
        screen.set(format!("create.{key}.tip"), (*tip).to_owned());
    }
}
