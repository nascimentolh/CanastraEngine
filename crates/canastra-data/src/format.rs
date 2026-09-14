//! On-disk game data (`.cana`): `CANASTRA`, a little-endian `u32` format version, then postcard.

use std::fmt;

use crate::GameData;

const MAGIC: [u8; 8] = *b"CANASTRA";
/// Bump on any change to a serialized type; the `layout_is_frozen` test fails until you do.
const VERSION: u32 = 1;

#[derive(Debug)]
pub enum FormatError {
    NotGameData,
    UnsupportedVersion(u32),
    Corrupt(postcard::Error),
    TrailingBytes(usize),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotGameData => f.write_str("not a Canastra game data file"),
            Self::UnsupportedVersion(v) => write!(f, "game data format {v} is not supported (expected {VERSION})"),
            Self::Corrupt(error) => write!(f, "game data is corrupt: {error}"),
            Self::TrailingBytes(len) => write!(f, "{len} unexpected bytes after the game data"),
        }
    }
}

impl std::error::Error for FormatError {}

/// # Panics
///
/// Never: every game data type serializes into memory.
pub fn encode(data: &GameData) -> Vec<u8> {
    let header = [MAGIC.as_slice(), &VERSION.to_le_bytes()].concat();
    postcard::to_extend(data, header).expect("serializing game data into memory cannot fail")
}

pub fn decode(bytes: &[u8]) -> Result<GameData, FormatError> {
    let (version, payload) =
        bytes.strip_prefix(&MAGIC).and_then(<[u8]>::split_first_chunk::<4>).ok_or(FormatError::NotGameData)?;
    match u32::from_le_bytes(*version) {
        VERSION => {}
        other => return Err(FormatError::UnsupportedVersion(other)),
    }
    let (data, rest) = postcard::take_from_bytes(payload).map_err(FormatError::Corrupt)?;
    match rest.len() {
        0 => Ok(data),
        len => Err(FormatError::TrailingBytes(len)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::asset::AssetRef;
    use crate::id::{ItemId, NpcId, SkillId, SkillRef};
    use crate::item::{
        Armor, Attachment, Body, BodyModel, HeldModel, Item, ItemKind, ItemModel, ModelPart, Stat, StatModifier,
        StatOp, WornModel,
    };
    use crate::npc::{Collision, Decoration, Npc, NpcFlags, NpcType, NpcVisual, Race, Sex};
    use crate::skill::{Skill, SkillLevel, SkillOperate, SkillSounds, SoundCue};
    use crate::text::Localized;

    const FROZEN: &[u8] = include_bytes!("../tests/format_v1.cana");

    fn asset<K>(path: &str) -> AssetRef<K> {
        AssetRef::parse(path).unwrap()
    }

    /// One value of every nested type, so a changed field anywhere changes the bytes.
    fn sample() -> GameData {
        let mut sword = Item::new(ItemId(1));
        sword.name = Localized::en("Sword");
        sword.skills = vec![SkillRef { id: SkillId(3), level: 1 }];
        sword.stats = vec![StatModifier { stat: Stat::PAtk, op: StatOp::Enchant, value: 4.5, order: Some(16) }];
        sword.visual.icon = Some(asset("icon.sword"));
        sword.visual.model = ItemModel::Held(HeldModel {
            parts: vec![ModelPart { mesh: Some(asset("W.sword")), textures: vec![asset("T.sword")] }],
            effect: Some(asset("E.glow")),
            grip: 1,
        });

        let mut plate = Item::new(ItemId(2));
        plate.kind = ItemKind::Armor(Armor::default());
        let body = BodyModel {
            meshes: vec![asset("M.body")],
            attachments: vec![Attachment { mesh: asset("M.wings"), params: [0, -1] }],
            extra_texture: Some(asset("T.extra")),
            ..BodyModel::default()
        };
        plate.visual.model =
            ItemModel::Worn(WornModel { bodies: BTreeMap::from([(Body::KamaelFemale, body)]), hit_effect: None });

        let level = SkillLevel {
            name: Localized::en("Power Strike"),
            icon: Some(asset("icon.skill0003")),
            animation: "S".into(),
            sounds: SkillSounds {
                spell: vec![SoundCue { sound: asset("S.cast"), volume: 250.0, radius: 80.0 }],
                cast_voices: BTreeMap::from([(Body::ElfMale, asset("S.voice"))]),
                ..SkillSounds::default()
            },
            ..SkillLevel::default()
        };
        let skill = Skill { id: SkillId(3), operate: SkillOperate::A1, levels: BTreeMap::from([(1, level)]) };

        let npc = Npc {
            id: NpcId(20006),
            name: Localized::en("Orc Archer"),
            title: Localized::default(),
            title_color: 0xFFFF_FFFF,
            npc_type: NpcType("Monster".into()),
            level: 5,
            race: Race::Orc,
            sex: Sex::Male,
            hp: 100.0,
            mp: 40.0,
            walk_speed: 20.0,
            run_speed: 110.0,
            collision: Collision { radius: 9.0, height: 23.5, grown: Some((10.0, 25.0)) },
            skills: vec![SkillRef { id: SkillId(4416), level: 6 }],
            flags: NpcFlags {
                attackable: true,
                targetable: true,
                talkable: false,
                show_name: true,
                undying: false,
                flying: false,
                can_move: true,
            },
            visual: NpcVisual {
                class: Some(asset("LineageMonster.orc_archer")),
                dialog_sounds: vec!["orc_greeting_1".into()],
                decorations: vec![Decoration { effect: asset("E.aura"), scale: 1.5 }],
                hp_bar: true,
                ..NpcVisual::default()
            },
        };

        GameData {
            items: BTreeMap::from([(ItemId(1), sword), (ItemId(2), plate)]),
            skills: BTreeMap::from([(SkillId(3), skill)]),
            npcs: BTreeMap::from([(NpcId(20006), npc)]),
        }
    }

    #[test]
    fn round_trips() {
        let data = sample();
        assert_eq!(decode(&encode(&data)).unwrap(), data);
    }

    /// Set `CANASTRA_BLESS=1` to rewrite the frozen file after bumping `VERSION`.
    #[test]
    fn layout_is_frozen() {
        let bytes = encode(&sample());
        if std::env::var_os("CANASTRA_BLESS").is_some() {
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/format_v1.cana"), &bytes).unwrap();
        }
        assert!(bytes == FROZEN, "serialized layout changed: bump format::VERSION and rebless the fixture");
    }

    #[test]
    fn rejects_foreign_and_damaged_files() {
        let bytes = encode(&sample());
        assert!(matches!(decode(b"OggS"), Err(FormatError::NotGameData)));
        let mut newer = bytes.clone();
        newer[8] = 2;
        assert!(matches!(decode(&newer), Err(FormatError::UnsupportedVersion(2))));
        assert!(matches!(decode(&bytes[..bytes.len() - 3]), Err(FormatError::Corrupt(_))));
        assert!(matches!(decode(&[bytes.as_slice(), &[0]].concat()), Err(FormatError::TrailingBytes(1))));
    }
}
