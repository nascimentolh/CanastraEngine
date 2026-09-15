//! On-disk game data (`.cana`): `CANASTRA`, a little-endian `u32` format version, then postcard.

use std::fmt;

use crate::GameData;

const MAGIC: [u8; 8] = *b"CANASTRA";
/// Bump on any change to a serialized type; the `layout_is_frozen` test fails until you do.
const VERSION: u32 = 3;

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
    use crate::appearance::{BodyLook, DisplayCharacter, HairStyle, Lobby, Look, Stand};
    use crate::asset::AssetRef;
    use crate::class::{
        Archetype, Attributes, BodySize, ClassTemplate, CombatBase, DamageRange, InitialItem, LevelGain, MDefSlots,
        MoveSpeed, Origin, PDefSlots, PlayerClass, StartingClass,
    };
    use crate::id::{ClassId, ItemId, NpcId, SkillId, SkillRef};
    use crate::item::WeaponType;
    use crate::item::{
        Armor, Attachment, Body, BodyModel, HeldModel, Item, ItemKind, ItemModel, ModelPart, Stat, StatModifier,
        StatOp, WornModel,
    };
    use crate::npc::{Collision, Decoration, Npc, NpcFlags, NpcType, NpcVisual, Race, Sex};
    use crate::skill::{Skill, SkillLevel, SkillOperate, SkillSounds, SoundCue};
    use crate::text::Localized;

    const FROZEN: &[u8] = include_bytes!("../tests/format_v3.cana");

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

        let class = starting_class();
        let (bodies, lobby) = appearance();

        GameData {
            items: BTreeMap::from([(ItemId(1), sword), (ItemId(2), plate)]),
            skills: BTreeMap::from([(SkillId(3), skill)]),
            npcs: BTreeMap::from([(NpcId(20006), npc)]),
            classes: BTreeMap::from([
                (ClassId(124), class),
                (
                    ClassId(125),
                    PlayerClass {
                        id: ClassId(125),
                        name: Localized::en("Trooper"),
                        levels: Vec::new(),
                        origin: Origin::Advanced { parent: ClassId(124) },
                    },
                ),
            ]),
            bodies,
            lobby,
        }
    }

    fn appearance() -> (BTreeMap<Body, BodyLook>, Lobby) {
        let bodies = BTreeMap::from([(
            Body::KamaelFemale,
            BodyLook {
                faces: vec![Look { mesh: asset("Kamael.FKamael_m000_f"), textures: vec![asset("FKamael.face")] }],
                hair_styles: vec![HairStyle {
                    front: None,
                    back: Some(Look { mesh: asset("K.bh"), textures: Vec::new() }),
                }],
                upper: Some(Look { mesh: asset("K.u"), textures: vec![asset("K.u"), asset("K.ut")] }),
                ..BodyLook::default()
            },
        )]);
        let lobby = Lobby {
            select: vec![Stand { location: [150_995.0, -246_683.0, -8117.0], yaw: -26408 }],
            creation: vec![DisplayCharacter {
                body: Body::KamaelFemale,
                archetype: Archetype::Fighter,
                stand: Stand { location: [174_548.0, -248_303.0, -10_356.0], yaw: -14868 },
                gear: vec![ItemId(2)],
            }],
        };
        (bodies, lobby)
    }

    fn starting_class() -> PlayerClass {
        let template = ClassTemplate {
            attributes: Attributes { str: 39, dex: 35, con: 30, int: 28, wit: 11, men: 27 },
            combat: CombatBase {
                p_atk: 4.0,
                m_atk: 6.0,
                critical_rate: 4.0,
                attack_type: WeaponType::Fist,
                p_atk_speed: 300.0,
                p_def: PDefSlots {
                    chest: 31.0,
                    legs: 18.0,
                    head: 12.0,
                    feet: 7.0,
                    gloves: 8.0,
                    underwear: 3.0,
                    cloak: 1.0,
                },
                m_def: MDefSlots { right_ear: 9.0, left_ear: 9.0, right_finger: 5.0, left_finger: 5.0, neck: 13.0 },
                penetrates: false,
                attack_range: 20,
                damage_range: DamageRange { vertical: 0, horizontal: 0, distance: 26, width: 120 },
                random_damage: 10,
            },
            move_speed: MoveSpeed { walk: 87.0, run: 122.0, slow_swim: 50.0, fast_swim: 50.0 },
            breath: 100,
            safe_fall: 500,
            body_male: BodySize { radius: 7.0, height: 22.6 },
            body_female: BodySize { radius: 7.0, height: 22.6 },
        };
        PlayerClass {
            id: ClassId(124),
            name: Localized::en("Female Soldier"),
            levels: vec![LevelGain { hp: 95.0, mp: 40.0, cp: 50.0, hp_regen: 2.0, mp_regen: 0.9, cp_regen: 2.0 }],
            origin: Origin::Starting(Box::new(StartingClass {
                race: Race::Kamael,
                archetype: Archetype::Fighter,
                sex: Some(Sex::Female),
                template,
                creation_points: vec![[-125_607, 38_452, 1152]],
                initial_items: vec![InitialItem {
                    item: ItemId(1),
                    count: 1,
                    equipped: true,
                    lasts_minutes: Some(2880),
                }],
            })),
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
            std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/format_v3.cana"), &bytes).unwrap();
        }
        assert!(bytes == FROZEN, "serialized layout changed: bump format::VERSION and rebless the fixture");
    }

    #[test]
    fn rejects_foreign_and_damaged_files() {
        let bytes = encode(&sample());
        assert!(matches!(decode(b"OggS"), Err(FormatError::NotGameData)));
        let mut newer = bytes.clone();
        newer[8] = 4;
        assert!(matches!(decode(&newer), Err(FormatError::UnsupportedVersion(4))));
        assert!(matches!(decode(&bytes[..bytes.len() - 3]), Err(FormatError::Corrupt(_))));
        assert!(matches!(decode(&[bytes.as_slice(), &[0]].concat()), Err(FormatError::TrailingBytes(1))));
    }
}
