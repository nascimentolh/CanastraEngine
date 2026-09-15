//! The characters the lobby stands in its scenes, dressed from their bodies in the game data.

use canastra_data::GameData;
use canastra_data::appearance::{Look, Stand, body_of};
use canastra_data::asset::TextureRef;
use canastra_data::class::Archetype;
use canastra_data::id::ItemId;
use canastra_data::item::{Body, EquipSlot, Item, ItemKind, ItemModel};
use canastra_data::npc::Race;
use canastra_protocol::game::{Appearance, CharacterSummary, Sex};

use crate::scene::{Figure, HeldSource, PartSource};

/// The client's grip of bows, which hang from the left hand.
const BOW_GRIP: u32 = 5;
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
            let start = data.starting_class(character.class)?;
            let body = body_of(start.race, start.archetype, character.sex == Sex::Female)?;
            let appearance = &character.appearance;
            let figure = figure(data, body, [appearance.face, appearance.hair_style].map(usize::from), &[], stand)?;
            Some(Figure { label: Some(character.name.clone()), ..figure })
        })
        .collect()
}

/// The characters on display for `race` at creation, in their display gear; the one of the `chosen` archetype and
/// sex wears `look`.
// ponytail: held weapons are left out until parts attach to bones; hair color waits for how H5 tints hair.
pub(super) fn creation(data: &GameData, race: Race, chosen: Option<(Archetype, Sex)>, look: Appearance) -> Vec<Figure> {
    data.lobby
        .creation
        .iter()
        .filter_map(|shown| {
            let sex = [Sex::Male, Sex::Female]
                .into_iter()
                .find(|&sex| body_of(race, shown.archetype, sex == Sex::Female) == Some(shown.body))?;
            let picked = chosen == Some((shown.archetype, sex));
            let [face, hair] = if picked { [look.face, look.hair_style].map(usize::from) } else { [0, 0] };
            figure(data, shown.body, [face, hair], &shown.gear, shown.stand)
        })
        .collect()
}

/// The slot of the character at `index`: the last for the selected one, the others in order for the rest.
fn slot(slots: &[Stand], index: usize, selected: usize) -> Option<&Stand> {
    let (center, others) = slots.split_last()?;
    if index == selected { Some(center) } else { others.get(if index < selected { index } else { index - 1 }) }
}

/// `body` with its face and hair style, wearing `gear` where it has a model for the body and bare parts elsewhere.
fn figure(data: &GameData, body: Body, [face, hair]: [usize; 2], gear: &[ItemId], stand: Stand) -> Option<Figure> {
    let look = data.bodies.get(&body)?;
    let hair = look.hair_styles.get(hair);
    let mut slots = [
        (EquipSlot::Gloves, look.gloves.as_ref().map(part).into_iter().collect()),
        (EquipSlot::Chest, look.upper.as_ref().map(part).into_iter().collect()),
        (EquipSlot::Legs, look.lower.as_ref().map(part).into_iter().collect()),
        (EquipSlot::Feet, look.boots.as_ref().map(part).into_iter().collect()),
    ];
    // Full armor takes the legs slot too: once one is worn, a legs item or another full armor after it is not.
    // The creation table lists robes in the legs slot of fighters in full armor.
    let mut legs_taken = false;
    for item in gear.iter().filter_map(|id| data.items.get(id)) {
        let (ItemKind::Armor(armor), ItemModel::Worn(worn)) = (&item.kind, &item.visual.model) else { continue };
        if legs_taken && matches!(armor.slot, EquipSlot::Legs | EquipSlot::FullArmor) {
            continue;
        }
        legs_taken |= armor.slot == EquipSlot::FullArmor;
        let Some(model) = worn.bodies.get(&body) else { continue };
        let worn = model
            .meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| PartSource {
                mesh: mesh.path().to_owned(),
                textures: section_textures(&model.textures, model.meshes.len(), index),
            })
            // Extra meshes worn with the armor, such as Kamael wings, each with the texture at its index.
            .chain(model.attachments.iter().enumerate().map(|(index, attachment)| PartSource {
                mesh: attachment.mesh.path().to_owned(),
                textures:
                    model.attachment_textures.get(index).map(|texture| texture.path().to_owned()).into_iter().collect(),
            }))
            .collect::<Vec<_>>();
        // Full armor wears its meshes over the chest and bares nothing of the legs.
        let full = armor.slot == EquipSlot::FullArmor;
        let mut worn = Some(worn);
        for (slot, parts) in &mut slots {
            if *slot == armor.slot || full && *slot == EquipSlot::Chest {
                *parts = worn.take().unwrap_or_default();
            } else if full && *slot == EquipSlot::Legs {
                parts.clear();
            }
        }
    }
    let items: Vec<&Item> = gear.iter().filter_map(|id| data.items.get(id)).collect();
    let in_hands = items.iter().flat_map(|item| held(item)).collect();
    let grip = items.iter().find_map(|item| match &item.visual.model {
        ItemModel::Held(model) if model.grip != 0 => Some(model.grip),
        _ => None,
    });
    let head =
        [look.faces.get(face), hair.and_then(|style| style.front.as_ref()), hair.and_then(|style| style.back.as_ref())];
    let parts = head.into_iter().flatten().map(part).chain(slots.into_iter().flat_map(|(_, parts)| parts)).collect();
    Some(Figure { parts, held: in_hands, location: stand.location, yaw: stand.yaw, sequence: idle(grip), label: None })
}

/// The idle a character plays holding a weapon of the client's `grip`, or none. Grips by `Weapongrp`: 1 one-handed,
/// 2 two-handed, 3 dual swords, 4 poles and staves, 5 bows, 6 and 9 daggers and rapiers, 7 fists and 10 paired
/// weapons (two meshes each), 8 crossbows.
fn idle(grip: Option<u32>) -> &'static str {
    match grip {
        Some(1 | 6 | 9) => "Wait_1HS",
        Some(2) => "Wait_2HS",
        Some(3 | 7 | 10) => "Wait_Dual",
        Some(4) => "Wait_Pole",
        Some(BOW_GRIP) => "Wait_Bow",
        Some(8) => "Wait_BowGun",
        _ => "Wait_Hand",
    }
}

/// What `item` puts in the character's hands: a shield on the left arm, a bow in the left hand, and weapons in
/// the right hand, a paired weapon's second piece in the left.
fn held(item: &Item) -> Vec<HeldSource> {
    let ItemModel::Held(model) = &item.visual.model else { return Vec::new() };
    let slot = match &item.kind {
        ItemKind::Weapon(weapon) => weapon.slot,
        ItemKind::Armor(armor) => armor.slot,
        ItemKind::Etc(etc) => etc.slot,
    };
    model
        .parts
        .iter()
        .enumerate()
        .filter_map(|(index, part)| {
            let bone = match (slot, model.grip, index) {
                (EquipSlot::LeftHand, _, _) => "Shield_L_Bone",
                (_, BOW_GRIP, _) | (_, _, 1) => "Weapon_L_Bone",
                _ => "Weapon_R_Bone",
            };
            let textures = part.textures.iter().map(|texture| texture.path().to_owned()).collect();
            Some(HeldSource { mesh: part.mesh.as_ref()?.path().to_owned(), textures, bone })
        })
        .collect()
}

/// The textures of the sections of mesh `index` of `count`, first section first. A mesh wears the texture at
/// its index, and the textures past the meshes dress further sections of the mesh whose texture names them:
/// Kamael armor lists `_t84_u`, `_t84_l`, then `_t84_ut` for the upper mesh's second section. A lone mesh
/// wears them all.
fn section_textures(textures: &[TextureRef], count: usize, index: usize) -> Vec<String> {
    let path = |texture: &TextureRef| texture.path().to_owned();
    if count == 1 {
        return textures.iter().map(path).collect();
    }
    let Some(own) = textures.get(index) else { return Vec::new() };
    let own_name = own.path().to_ascii_lowercase();
    let extras = textures.iter().skip(count).filter(|extra| extra.path().to_ascii_lowercase().starts_with(&own_name));
    std::iter::once(own).chain(extras).map(path).collect()
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

    #[test]
    fn full_armor_replaces_the_bare_chest_and_legs_and_keeps_the_rest() {
        use canastra_data::appearance::BodyLook;
        use canastra_data::asset::AssetRef;
        use canastra_data::item::{Armor, BodyModel, Item, WornModel};

        let look = |mesh: &str| Some(Look { mesh: AssetRef::parse(mesh).unwrap(), textures: Vec::new() });
        let bare = BodyLook { upper: look("M.u"), lower: look("M.l"), boots: look("M.b"), ..BodyLook::default() };
        let mut robe = Item::new(ItemId(1));
        robe.kind = ItemKind::Armor(Armor { slot: EquipSlot::FullArmor, ..Armor::default() });
        let model = BodyModel {
            meshes: vec![AssetRef::parse("M.robe_u").unwrap(), AssetRef::parse("M.robe_l").unwrap()],
            textures: vec![AssetRef::parse("T.robe_u").unwrap(), AssetRef::parse("T.robe_l").unwrap()],
            ..BodyModel::default()
        };
        robe.visual.model = ItemModel::Worn(WornModel { bodies: [(Body::ElfMale, model)].into(), hit_effect: None });
        let mut data = GameData::default();
        data.bodies.insert(Body::ElfMale, bare);
        data.items.insert(ItemId(1), robe);

        let stand = Stand { location: [0.0; 3], yaw: 0 };
        let figure = figure(&data, Body::ElfMale, [0, 0], &[ItemId(1)], stand).unwrap();

        let parts: Vec<(&str, Vec<&str>)> = figure
            .parts
            .iter()
            .map(|part| (part.mesh.as_str(), part.textures.iter().map(String::as_str).collect()))
            .collect();
        assert_eq!(parts, [("M.robe_u", vec!["T.robe_u"]), ("M.robe_l", vec!["T.robe_l"]), ("M.b", vec![])]);
    }

    #[test]
    fn extra_textures_dress_the_sections_of_the_mesh_they_extend() {
        use canastra_data::asset::AssetRef;

        let textures: Vec<TextureRef> =
            ["K.t84_u", "K.t84_l", "K.t84_ut"].iter().map(|path| AssetRef::parse(path).unwrap()).collect();
        assert_eq!(section_textures(&textures, 2, 0), ["K.t84_u", "K.t84_ut"]);
        assert_eq!(section_textures(&textures, 2, 1), ["K.t84_l"]);
    }
}
