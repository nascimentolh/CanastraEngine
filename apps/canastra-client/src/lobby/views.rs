//! The creation scene's camera views and the routes between them, as Lobby02 names its scenes: `Elf` shows the
//! race, `Elf_Knight` flies to its fighters, `Elf_Knight_Kman` on to the male one, `Elf_Kman_Kwoman` cuts to the
//! female one and `Elf_Kman_Chest` closes in on him.

use canastra_data::class::Archetype;
use canastra_protocol::game::Sex;

use crate::scene::Route;

/// The view the creation choices call for: empty for the race, `Knight` or `Wizard` for a class's pair, `Kman`,
/// `Kwoman`, `Wman` or `Wwoman` for one character, and that character's name with `_Chest` close up.
pub(super) fn view(archetype: Option<Archetype>, sex: Option<Sex>, zoomed: bool) -> String {
    let Some(archetype) = archetype else { return String::new() };
    let (pair, letter) = match archetype {
        Archetype::Fighter => ("Knight", "K"),
        Archetype::Mystic => ("Wizard", "W"),
    };
    let Some(sex) = sex else { return pair.to_owned() };
    let who = format!("{letter}{}", if sex == Sex::Male { "man" } else { "woman" });
    if zoomed { format!("{who}_Chest") } else { who }
}

/// The routes from view `from` to view `to` of `race`'s scenes, best first; the last cuts straight to `to`.
pub(super) fn routes(race: &str, from: &str, to: &str) -> Vec<Route> {
    let tag = |views: &[&str]| {
        std::iter::once(race).chain(views.iter().copied().filter(|view| !view.is_empty())).collect::<Vec<_>>().join("_")
    };
    let pair = |view: &str| match view.chars().next() {
        Some('K') => "Knight",
        Some('W') => "Wizard",
        _ => "",
    };
    let mut routes = Vec::new();
    if !from.is_empty() && !to.is_empty() {
        routes.extend([Route::Play(tag(&[from, to])), Route::Back(tag(&[to, from]))]);
    }
    // Back to the race, or out of a close-up, the way the view was reached.
    if to.is_empty() || from.strip_suffix("_Chest") == Some(to) {
        routes.push(Route::Back(tag(&[from])));
    }
    // One character is reached from their class's pair; the race, a pair and a close-up by their own scenes.
    let single = !matches!(to, "" | "Knight" | "Wizard") && !to.ends_with("_Chest");
    let arrival = if single { tag(&[pair(to), to]) } else { tag(&[to]) };
    routes.extend([Route::Play(arrival.clone()), Route::Cut(arrival)]);
    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_pick_views_and_views_pick_lobby_scenes() {
        assert_eq!(view(None, Some(Sex::Male), true), "");
        assert_eq!(view(Some(Archetype::Mystic), None, true), "Wizard");
        assert_eq!(view(Some(Archetype::Fighter), Some(Sex::Female), true), "Kwoman_Chest");

        let first = |from, to| routes("Elf", from, to).into_iter().next();
        assert_eq!(first("", "Knight"), Some(Route::Play("Elf_Knight".into())));
        assert_eq!(first("Knight", ""), Some(Route::Back("Elf_Knight".into())));
        assert!(routes("Elf", "Kman", "Kman_Chest").contains(&Route::Play("Elf_Kman_Chest".into())));
        assert!(routes("Elf", "Kman_Chest", "Kman").contains(&Route::Back("Elf_Kman_Chest".into())));
        assert_eq!(routes("Elf", "Kman", "Wman").last(), Some(&Route::Cut("Elf_Wizard_Wman".into())));
    }
}
