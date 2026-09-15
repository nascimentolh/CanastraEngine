//! The characters the lobby stands in its scenes, dressed from their bodies in the game data.

use canastra_data::GameData;
use canastra_data::appearance::{Look, Stand, body_of};
use canastra_protocol::game::{CharacterSummary, Sex};

use crate::scene::{Figure, PartSource};

/// The idle sequence characters loop in the lobby.
const IDLE: &str = "Wait_Hand";
/// How far behind the last slot H5 stands the selected character, on the rune circle's far edge. Measured
/// from an H5 screenshot, where the other slots fall exactly where the camera puts them.
const SELECTED_STEP_BACK: f32 = 65.0;

/// The account's characters on the select slots, leaving out those the data cannot dress.
// ponytail: bare bodies only; worn gear shows once the character list carries equipment.
pub(super) fn select(data: &GameData, characters: &[CharacterSummary], selected: usize) -> Vec<Figure> {
    characters
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            let mut stand = *slot(&data.lobby.select, index, selected)?;
            if index == selected {
                let yaw = stand.yaw as f32 / 65536.0 * std::f32::consts::TAU;
                stand.location[0] -= yaw.cos() * SELECTED_STEP_BACK;
                stand.location[1] -= yaw.sin() * SELECTED_STEP_BACK;
            }
            figure(data, character, stand)
        })
        .collect()
}

/// The slot of the character at `index`: the last for the selected one, the others in order for the rest.
fn slot(slots: &[Stand], index: usize, selected: usize) -> Option<&Stand> {
    let (center, others) = slots.split_last()?;
    if index == selected { Some(center) } else { others.get(if index < selected { index } else { index - 1 }) }
}

fn figure(data: &GameData, character: &CharacterSummary, stand: Stand) -> Option<Figure> {
    let start = data.starting_class(character.class)?;
    let body = data.bodies.get(&body_of(start.race, start.archetype, character.sex == Sex::Female)?)?;
    let hair = body.hair_styles.get(usize::from(character.appearance.hair_style));
    let looks = [
        body.faces.get(usize::from(character.appearance.face)),
        hair.and_then(|style| style.front.as_ref()),
        hair.and_then(|style| style.back.as_ref()),
        body.gloves.as_ref(),
        body.upper.as_ref(),
        body.lower.as_ref(),
        body.boots.as_ref(),
    ];
    let parts = looks.into_iter().flatten().map(part).collect();
    Some(Figure { parts, location: stand.location, yaw: stand.yaw, sequence: IDLE })
}

fn part(look: &Look) -> PartSource {
    PartSource {
        mesh: look.mesh.path().to_owned(),
        textures: look.textures.iter().map(|texture| texture.path().to_owned()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selected_character_takes_the_last_slot_and_the_others_keep_their_order() {
        let slots: Vec<Stand> = (0..4).map(|yaw| Stand { location: [0.0; 3], yaw }).collect();
        let yaws: Vec<Option<i32>> = (0..5).map(|index| slot(&slots, index, 1).map(|stand| stand.yaw)).collect();
        assert_eq!(yaws, [Some(0), Some(3), Some(1), Some(2), None]);
    }
}
