//! Characters' bare bodies from the client's `Chargrp`, and where the lobby stands characters, from `Logongrp`
//! (character select) and `Charcreategrp` (character creation).

use std::collections::BTreeMap;

use canastra_data::appearance::{BodyLook, DisplayCharacter, HairStyle, Lobby, Look, Stand};
use canastra_data::class::Archetype;
use canastra_data::id::ItemId;
use canastra_data::item::Body;
use l2_dat::Value;

use crate::fields::{Assets, Fields, Result};
use crate::{Report, Severity, Sources};

/// The body of each `Chargrp` record, in file order. A seventeenth record is empty.
const BODIES: [Body; 16] = [
    Body::HumanFighterMale,
    Body::HumanFighterFemale,
    Body::DarkElfMale,
    Body::DarkElfFemale,
    Body::DwarfMale,
    Body::DwarfFemale,
    Body::ElfMale,
    Body::ElfFemale,
    Body::HumanMysticMale,
    Body::HumanMysticFemale,
    Body::OrcFighterMale,
    Body::OrcFighterFemale,
    Body::OrcMysticMale,
    Body::OrcMysticFemale,
    Body::KamaelMale,
    Body::KamaelFemale,
];

/// Who each `Charcreategrp` record shows: four per race (male and female fighter, then male and female
/// mystic) for Human, Elf, Dark Elf and Orc, then the Dwarf and Kamael pairs.
const DISPLAY: [(Body, Archetype); 20] = [
    (Body::HumanFighterMale, Archetype::Fighter),
    (Body::HumanFighterFemale, Archetype::Fighter),
    (Body::HumanMysticMale, Archetype::Mystic),
    (Body::HumanMysticFemale, Archetype::Mystic),
    (Body::ElfMale, Archetype::Fighter),
    (Body::ElfFemale, Archetype::Fighter),
    (Body::ElfMale, Archetype::Mystic),
    (Body::ElfFemale, Archetype::Mystic),
    (Body::DarkElfMale, Archetype::Fighter),
    (Body::DarkElfFemale, Archetype::Fighter),
    (Body::DarkElfMale, Archetype::Mystic),
    (Body::DarkElfFemale, Archetype::Mystic),
    (Body::OrcFighterMale, Archetype::Fighter),
    (Body::OrcFighterFemale, Archetype::Fighter),
    (Body::OrcMysticMale, Archetype::Mystic),
    (Body::OrcMysticFemale, Archetype::Mystic),
    (Body::DwarfMale, Archetype::Fighter),
    (Body::DwarfFemale, Archetype::Fighter),
    (Body::KamaelMale, Archetype::Fighter),
    (Body::KamaelFemale, Archetype::Fighter),
];

/// The hair table holds five blocks of fifteen styles; the first is the hair worn without headgear.
const HAIR_STYLES: usize = 15;

/// Sounds, effects and animation timing come with combat; the `_add` and `tab_byte` columns and fillers
/// have no known meaning.
const UNMODELED: &[&str] = &[
    "filler_01",
    "filler_02",
    "glove_mesh_add",
    "glove_tex_add",
    "glove_tab_byte1",
    "glove_tab_byte2",
    "upper_mesh_add",
    "upper_tex_add",
    "upper_tab_byte1",
    "upper_tab_bytee2",
    "upper_tab_byte2",
    "lower_mesh_add",
    "lower_tex_add",
    "lower_tab_byte1",
    "lower_tab_byte2",
    "boot_mesh_add",
    "boot_tex_add",
    "boot_tab_byte1",
    "boot_tab_byte2",
    "attack_effect",
    "walkanimframe",
    "attack_sound",
    "defense_sound",
    "damage_sound",
    "voice_snd_hand",
    "voice_snd_1hs",
    "voice_snd_2hs",
    "voice_snd_dual",
    "voice_snd_pole",
    "voice_snd_reserve1",
    "voice_snd_reserve2",
    "voice_snd_reserve3",
    "voice_snd_reserve4",
    "voice_snd_reserve5",
    "voice_snd_reserve6",
    "final",
    "name",
    "unk_00",
    "unk_01",
    "unk_02",
    "unk_03",
    "p1",
    "p2",
];

pub(crate) fn bodies(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<Body, BodyLook> {
    let mut bodies = BTreeMap::new();
    for (&body, record) in BODIES.iter().zip(sources.chargrp) {
        let mut assets = Assets::default();
        match body_look(record, &mut assets) {
            Ok(look) => {
                bodies.insert(body, look);
            }
            Err(error) => report.push(Severity::Error, format!("body {body:?}"), error),
        }
        for note in assets.notes {
            report.push(Severity::Warning, format!("body {body:?}"), note);
        }
    }
    bodies
}

pub(crate) fn lobby(sources: &Sources<'_>, report: &mut Report) -> Lobby {
    let mut lobby = Lobby::default();
    for (index, record) in sources.logongrp.iter().enumerate() {
        match stand(record, ["x", "y", "z", "yaw"]).and_then(|(stand, fields)| fields.finish().map(|()| stand)) {
            Ok(stand) => lobby.select.push(stand),
            Err(error) => report.push(Severity::Error, format!("select slot {index}"), error),
        }
    }
    for (&(body, archetype), record) in DISPLAY.iter().zip(sources.charcreategrp) {
        let shown = stand(record, ["flts_1", "flts_2", "flts_3", "flts_4"]).and_then(|(stand, mut fields)| {
            let gear = ["chest", "legs", "gloves", "feet", "rhand", "lhand"]
                .iter()
                .map(|slot| fields.uint(slot))
                .collect::<Result<Vec<u32>>>()?;
            fields.finish()?;
            let gear = gear.into_iter().filter(|&id| id != 0).map(ItemId).collect();
            Ok(DisplayCharacter { body, archetype, stand, gear })
        });
        match shown {
            Ok(shown) => lobby.creation.push(shown),
            Err(error) => report.push(Severity::Error, format!("creation display {body:?} {archetype:?}"), error),
        }
    }
    lobby
}

fn body_look(record: &Value, assets: &mut Assets) -> Result<BodyLook> {
    let mut fields = Fields::of(record)?;
    let hair = fields.list("hair_tab")?;
    let mut hair_styles = hair
        .iter()
        .take(HAIR_STYLES)
        .map(|style| {
            let mut style = Fields::of(style)?;
            let front = look(assets, style.text("ahair_mesh0")?, &[style.text("ahair_texture0")?]);
            let back = look(assets, style.text("bhair_mesh0")?, &[style.text("bhair_texture0")?]);
            style.finish()?;
            Ok(HairStyle { front, back })
        })
        .collect::<Result<Vec<_>>>()?;
    // The table pads each body to fifteen styles.
    while hair_styles.last().is_some_and(|style| style.front.is_none() && style.back.is_none()) {
        hair_styles.pop();
    }
    let face_meshes = fields.texts("face_mesh")?;
    let face_textures = fields.texts("face_texture")?;
    let faces =
        face_meshes.iter().zip(&face_textures).filter_map(|(mesh, texture)| look(assets, mesh, &[texture])).collect();
    let mut part = |mesh: &str, texture: &str| -> Result<Option<Look>> {
        let (meshes, textures) = (fields.texts(mesh)?, fields.texts(texture)?);
        Ok(meshes.first().and_then(|mesh| look(assets, mesh, &textures)))
    };
    let gloves = part("glove_mesh", "glove_tex")?;
    let upper = part("upper_mesh", "upper_tex")?;
    let lower = part("lower_mesh", "lower_tex")?;
    let boots = part("boot_mesh", "boot_tex")?;
    fields.ignore(UNMODELED);
    fields.finish()?;
    Ok(BodyLook { faces, hair_styles, gloves, upper, lower, boots })
}

fn look(assets: &mut Assets, mesh: &str, textures: &[&str]) -> Option<Look> {
    Some(Look { mesh: assets.one(mesh)?, textures: assets.many(textures) })
}

fn stand<'a>(record: &'a Value, [x, y, z, yaw]: [&str; 4]) -> Result<(Stand, Fields<'a>)> {
    let mut fields = Fields::of(record)?;
    let location = [fields.float(x)?, fields.float(y)?, fields.float(z)?];
    #[expect(clippy::cast_possible_truncation, reason = "yaws are whole rotation units stored as floats")]
    let yaw = fields.float(yaw)? as i32;
    Ok((Stand { location, yaw }, fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Value {
        Value::Text(value.into())
    }

    fn texts(values: &[&str]) -> Value {
        Value::List(values.iter().map(|value| text(value)).collect())
    }

    #[test]
    fn a_body_keeps_its_faces_parts_and_hair_styles_without_the_padding() {
        let style = |front: &str, back: &str| {
            Value::Record(vec![
                ("ahair_mesh0", text(front)),
                ("ahair_texture0", text(if front.is_empty() { "" } else { "T.front" })),
                ("bhair_mesh0", text(back)),
                ("bhair_texture0", text(if back.is_empty() { "" } else { "T.back" })),
            ])
        };
        let mut hair = vec![style("M.a0", "M.b0"), style("", "M.b1")];
        hair.resize_with(75, || style("", ""));
        hair[20] = style("", "M.helmet");
        let record = Value::Record(vec![
            ("hair_tab", Value::List(hair)),
            ("face_mesh", texts(&["M.face", "M.face"])),
            ("face_texture", texts(&["T.face0", "T.face1"])),
            ("glove_mesh", texts(&["M.g"])),
            ("glove_tex", texts(&["T.g"])),
            ("upper_mesh", texts(&["M.u"])),
            ("upper_tex", texts(&["T.u", "T.ut"])),
            ("lower_mesh", texts(&[])),
            ("lower_tex", texts(&[])),
            ("boot_mesh", texts(&["M.b"])),
            ("boot_tex", texts(&["T.b"])),
            ("attack_sound", texts(&["S.hit"])),
        ]);

        let body = body_look(&record, &mut Assets::default()).unwrap();

        assert_eq!(body.faces.len(), 2);
        assert_eq!(body.faces[1].textures[0].path(), "T.face1");
        assert_eq!(body.hair_styles.len(), 2);
        assert!(body.hair_styles[1].front.is_none() && body.hair_styles[1].back.is_some());
        assert_eq!(body.upper.unwrap().textures.len(), 2);
        assert!(body.lower.is_none());
    }
}
