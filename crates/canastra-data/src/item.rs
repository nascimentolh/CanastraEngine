//! Items: gameplay rules and presentation of everything a character can own.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::asset::{EffectRef, MeshRef, SoundRef, TextureRef};
use crate::id::{ItemId, SkillRef};
use crate::text::Localized;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub name: Localized,
    /// Suffix shown after the name, e.g. a special ability.
    pub additional_name: Localized,
    pub description: Localized,
    pub weight: u32,
    pub material: Material,
    pub grade: Grade,
    /// Reference price in adena.
    pub price: u64,
    pub crystal_count: u32,
    pub flags: ItemFlags,
    /// What using the item does.
    pub action: ItemAction,
    /// Server behavior registered under this key; plugins may add their own.
    pub handler: Option<HandlerKey>,
    /// Skills granted while equipped or cast on use.
    pub skills: Vec<SkillRef>,
    pub stats: Vec<StatModifier>,
    pub reuse_delay_ms: u32,
    pub shared_reuse_group: Option<u32>,
    /// Mana of shadow items, in minutes.
    pub mana_minutes: Option<u32>,
    /// Lifetime of limited-time items, in minutes.
    pub lifetime_minutes: Option<u32>,
    pub kind: ItemKind,
    pub visual: ItemVisual,
}

impl Item {
    /// A plain item with default rules and no presentation.
    pub fn new(id: ItemId) -> Self {
        Self {
            id,
            name: Localized::default(),
            additional_name: Localized::default(),
            description: Localized::default(),
            weight: 0,
            material: Material::Steel,
            grade: Grade::None,
            price: 0,
            crystal_count: 0,
            flags: ItemFlags::default(),
            action: ItemAction::None,
            handler: None,
            skills: Vec::new(),
            stats: Vec::new(),
            reuse_delay_ms: 0,
            shared_reuse_group: None,
            mana_minutes: None,
            lifetime_minutes: None,
            kind: ItemKind::Etc(EtcItem::default()),
            visual: ItemVisual::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon(Weapon),
    Armor(Armor),
    Etc(EtcItem),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Weapon {
    pub weapon_type: WeaponType,
    pub slot: EquipSlot,
    pub soulshots: u32,
    pub spiritshots: u32,
    pub random_damage: u32,
    pub attack_range: u32,
    /// Server hit geometry, kept verbatim until combat is designed.
    pub damage_range: Option<[i32; 4]>,
    pub mp_consume: u32,
    pub magic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Armor {
    pub armor_type: ArmorType,
    pub slot: EquipSlot,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EtcItem {
    pub etc_type: EtcItemType,
    /// Some etc items are equipped, e.g. arrows in the left hand.
    pub slot: EquipSlot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[expect(clippy::struct_excessive_bools, reason = "independent rule switches edited as checkboxes")]
pub struct ItemFlags {
    pub stackable: bool,
    pub sellable: bool,
    pub droppable: bool,
    pub destroyable: bool,
    pub tradable: bool,
    pub depositable: bool,
    pub freightable: bool,
    pub enchantable: bool,
    pub elementable: bool,
    pub quest: bool,
    pub olympiad_restricted: bool,
    pub for_npc: bool,
    pub immediate_effect: bool,
    pub self_resurrection: bool,
}

impl Default for ItemFlags {
    fn default() -> Self {
        Self {
            stackable: false,
            sellable: true,
            droppable: true,
            destroyable: true,
            tradable: true,
            depositable: true,
            freightable: false,
            enchantable: false,
            elementable: false,
            quest: false,
            olympiad_restricted: false,
            for_npc: false,
            immediate_effect: false,
            self_resurrection: false,
        }
    }
}

/// Identifier of a server-side item behavior, e.g. `ItemSkills`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HandlerKey(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatModifier {
    pub stat: Stat,
    pub op: StatOp,
    pub value: f64,
    /// Explicit evaluation order; `None` uses the operation's default.
    pub order: Option<u32>,
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum StatOp {
        Set,
        Add,
        Sub,
        Mul,
        /// Bonus applied per enchant level.
        Enchant,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    pub enum Stat {
        MaxMp,
        PAtk,
        MAtk,
        PAtkSpeed,
        PDef,
        MDef,
        ShieldDef,
        ShieldRate,
        Evasion,
        CritRate,
        Accuracy,
        AttackRange,
        FireRes,
        WaterRes,
        WindRes,
        EarthRes,
        HolyRes,
        DarkRes,
        HolyPower,
        MagicSuccessRes,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum Grade {
        #[default]
        None,
        D,
        C,
        B,
        A,
        S,
        S80,
        S84,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    pub enum Material {
        Steel,
        FineSteel,
        BloodSteel,
        Bronze,
        Silver,
        Gold,
        Mithril,
        Oriharukon,
        Damascus,
        Adamantaite,
        Chrysolite,
        Crystal,
        Paper,
        Wood,
        Cloth,
        Cotton,
        Leather,
        Bone,
        Horn,
        Liquid,
        ScaleOfDragon,
        Dyestuff,
        Cobweb,
        Seed,
        Fish,
        RuneXp,
        RuneSp,
        RuneRemovePenalty,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum WeaponType {
        #[default]
        None,
        Sword,
        Blunt,
        Dagger,
        Bow,
        Pole,
        Dual,
        Etc,
        Fist,
        DualFist,
        FishingRod,
        Rapier,
        AncientSword,
        Crossbow,
        DualDagger,
        Flag,
        OwnThing,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum ArmorType {
        #[default]
        None,
        Light,
        Heavy,
        Magic,
        Sigil,
        Shield,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum EquipSlot {
        #[default]
        None,
        RightHand,
        LeftHand,
        BothHands,
        Head,
        Chest,
        Legs,
        Feet,
        Gloves,
        Underwear,
        Back,
        /// Chest and legs in one piece.
        FullArmor,
        AllDress,
        Neck,
        Ears,
        Fingers,
        Hair,
        /// Face accessory (`hair2`).
        Face,
        /// Covers hair and face.
        HairAll,
        LeftBracelet,
        RightBracelet,
        Talisman,
        Belt,
    }
}

impl EquipSlot {
    pub fn is_hand(self) -> bool {
        matches!(self, Self::RightHand | Self::LeftHand | Self::BothHands)
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum EtcItemType {
        #[default]
        None,
        Arrow,
        Bolt,
        Potion,
        Elixir,
        Scroll,
        EnchantWeapon,
        EnchantArmor,
        BlessedEnchantWeapon,
        BlessedEnchantArmor,
        AncientCrystalEnchantWeapon,
        AncientCrystalEnchantArmor,
        EnchantChanceWeapon,
        EnchantChanceArmor,
        EnchantAttribute,
        Recipe,
        Material,
        PetCollar,
        CastleGuard,
        Lotto,
        RaceTicket,
        Dye,
        Seed,
        Seed2,
        Crop,
        MatureCrop,
        Harvest,
        TicketOfLord,
        Lure,
        Coupon,
        Rune,
        RuneSelect,
        Shot,
    }
}

choices! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
    pub enum ItemAction {
        #[default]
        None,
        Equip,
        Calc,
        CallSkill,
        Capsule,
        CreateCommandChannel,
        Dice,
        FishingShot,
        Harvest,
        HideName,
        KeepExp,
        NickColor,
        Peel,
        Recipe,
        Seed,
        ShowAdventurerGuideBook,
        ShowHtml,
        ShowSevenSignsStatus,
        SkillMaintain,
        SkillReduce,
        Soulshot,
        Spiritshot,
        StartQuest,
        SummonSoulshot,
        SummonSpiritshot,
        ChristmasOpen,
    }
}

/// Everything the client needs to show an item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ItemVisual {
    pub icon: Option<TextureRef>,
    /// Icons of the individual pieces of paired weapons.
    pub part_icons: Vec<TextureRef>,
    /// Frame drawn behind the icon, e.g. for player-versus-player or limited-time items.
    pub icon_panel: Option<TextureRef>,
    pub drop: DropVisual,
    /// Swing, impact or wear sounds.
    pub sounds: Vec<SoundRef>,
    pub equip_sound: Option<SoundRef>,
    pub model: ItemModel,
}

/// The item lying on the ground.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DropVisual {
    pub meshes: Vec<MeshRef>,
    pub textures: Vec<TextureRef>,
    pub radius: u32,
    pub height: u32,
    pub sound: Option<SoundRef>,
}

/// How the item looks when equipped. Independent of gameplay kind: shields are
/// armor held like a weapon, and some etc items are worn meshes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum ItemModel {
    #[default]
    None,
    /// Meshes attached to the character, e.g. weapons, shields and cloaks.
    Held(HeldModel),
    /// Meshes authored per body, e.g. armor.
    Worn(WornModel),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HeldModel {
    pub parts: Vec<ModelPart>,
    pub effect: Option<EffectRef>,
    /// Client animation grip class; meaning is mapped when animation lands.
    pub grip: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ModelPart {
    pub mesh: Option<MeshRef>,
    pub textures: Vec<TextureRef>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct WornModel {
    pub bodies: BTreeMap<Body, BodyModel>,
    /// Played on the wearer when hit.
    pub hit_effect: Option<EffectRef>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BodyModel {
    pub meshes: Vec<MeshRef>,
    pub textures: Vec<TextureRef>,
    pub attachments: Vec<Attachment>,
    pub attachment_textures: Vec<TextureRef>,
    pub extra_texture: Option<TextureRef>,
}

/// Extra mesh worn with an armor piece, e.g. Kamael wings or shoulder guards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub mesh: MeshRef,
    /// The client's two letters for the mesh, as in its name's `_Hrs_` suffix, read backwards: where it hangs
    /// (`r`, `a`, `h`, `s`, `m`; `w` and `l` for Kamael wings and skirts), then whether it is rigid (`r`) or
    /// simulated cloth (`s`).
    pub params: [i8; 2],
}

impl Attachment {
    /// The body bone the mesh hangs from, for a mesh bound to one: its root bone stands on that bone in the bind
    /// pose. Kamael wings and skirts play animations of their own instead.
    pub fn bone(&self) -> Option<&'static str> {
        match u8::try_from(self.params[0]).ok()? {
            b'r' => Some("Bip01_R_UpperArm"),
            b'a' => Some("Bip01_L_UpperArm"),
            b'h' => Some("Shoulder_R_Bone"),
            b's' => Some("Shoulder_L_Bone"),
            b'm' => Some("Bip01_Spine2"),
            _ => None,
        }
    }
}

/// Character body a model is authored for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Body {
    HumanFighterMale,
    HumanFighterFemale,
    HumanMysticMale,
    HumanMysticFemale,
    ElfMale,
    ElfFemale,
    DarkElfMale,
    DarkElfFemale,
    OrcFighterMale,
    OrcFighterFemale,
    OrcMysticMale,
    OrcMysticFemale,
    DwarfMale,
    DwarfFemale,
    KamaelMale,
    KamaelFemale,
    Npc,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_letters_name_the_bone_they_hang_from() {
        let attachment = |letter: u8| Attachment {
            mesh: MeshRef::parse("Elf.MElf_m008_Lrr_ad00").unwrap(),
            params: [letter.cast_signed(), b'r'.cast_signed()],
        };
        assert_eq!(attachment(b'r').bone(), Some("Bip01_R_UpperArm"));
        assert_eq!(attachment(b's').bone(), Some("Shoulder_L_Bone"));
        assert_eq!(attachment(b'w').bone(), None, "Kamael wings animate on their own");
    }
}
