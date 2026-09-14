//! Item gameplay rules from reference server XML (`<item id type name>` with `<set>` and `<stats>`).

use std::collections::BTreeMap;
use std::str::FromStr;

use canastra_data::asset::TextureRef;
use canastra_data::id::{ItemId, SkillId, SkillRef};
use canastra_data::item::{
    Armor, ArmorType, EquipSlot, EtcItem, EtcItemType, Grade, HandlerKey, Item, ItemAction, ItemKind, Material, Stat,
    StatModifier, StatOp, Weapon, WeaponType,
};
use canastra_data::text::Localized;
use roxmltree::Node;

use crate::Report;
use crate::fields::{Result, parse};

/// `<set name val>` pairs still to be consumed.
type Sets<'a> = BTreeMap<&'a str, &'a str>;

pub(crate) fn item(node: Node<'_, '_>, report: &mut Report) -> Result<(ItemId, Item)> {
    let id = ItemId(parse("id", node.attribute("id").unwrap_or_default())?);
    let mut sets: Sets<'_> = node
        .children()
        .filter(|child| child.has_tag_name("set"))
        .filter_map(|set| Some((set.attribute("name")?, set.attribute("val")?)))
        .collect();

    let mut item = Item::new(id);
    item.name = Localized::en(node.attribute("name").unwrap_or_default());
    // Used when the client has no presentation for the item.
    item.visual.icon = sets.remove("icon").and_then(|path| TextureRef::parse(path).ok());
    rules(&mut item, &mut sets)?;
    item.kind = kind(node.attribute("type").unwrap_or_default(), &mut sets)?;

    for child in node.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "set" => {}
            "stats" => item.stats = stats(child)?,
            other => report.unmodeled(format!("<{other}>")),
        }
    }
    let item_type = node.attribute("type").unwrap_or_default();
    for name in sets.keys() {
        report.unmodeled(format!("set:{name} on {item_type}"));
    }
    Ok((id, item))
}

fn rules(item: &mut Item, sets: &mut Sets<'_>) -> Result<()> {
    item.weight = take(sets, "weight")?.unwrap_or(0);
    item.material = take_enum(sets, "material", material)?.unwrap_or(Material::Steel);
    item.grade = take_enum(sets, "crystal_type", grade)?.unwrap_or_default();
    item.price = take(sets, "price")?.unwrap_or(0);
    item.crystal_count = take(sets, "crystal_count")?.unwrap_or(0);
    item.action = take_enum(sets, "default_action", action)?.unwrap_or_default();
    item.handler = sets.remove("handler").map(|key| HandlerKey(key.to_owned()));
    item.skills = sets.remove("item_skill").map(skills).transpose()?.unwrap_or_default();
    item.reuse_delay_ms = take(sets, "reuse_delay")?.unwrap_or(0);
    item.shared_reuse_group = take(sets, "shared_reuse_group")?.filter(|&group: &u32| group != 0);
    item.mana_minutes = take::<i64>(sets, "duration")?.and_then(|minutes| u32::try_from(minutes).ok());
    item.lifetime_minutes = take::<i64>(sets, "time")?.and_then(|minutes| u32::try_from(minutes).ok());

    let flags = &mut item.flags;
    for (key, flag) in [
        ("is_stackable", &mut flags.stackable),
        ("is_sellable", &mut flags.sellable),
        ("is_dropable", &mut flags.droppable),
        ("is_destroyable", &mut flags.destroyable),
        ("is_tradable", &mut flags.tradable),
        ("is_depositable", &mut flags.depositable),
        ("is_freightable", &mut flags.freightable),
        ("enchant_enabled", &mut flags.enchantable),
        ("element_enabled", &mut flags.elementable),
        ("is_questitem", &mut flags.quest),
        ("is_oly_restricted", &mut flags.olympiad_restricted),
        ("for_npc", &mut flags.for_npc),
        ("immediate_effect", &mut flags.immediate_effect),
        ("allow_self_resurrection", &mut flags.self_resurrection),
    ] {
        if let Some(value) = take(sets, key)? {
            *flag = value;
        }
    }
    Ok(())
}

fn kind(item_type: &str, sets: &mut Sets<'_>) -> Result<ItemKind> {
    let slot = |sets: &mut Sets<'_>| take_enum(sets, "bodypart", slot).map(Option::unwrap_or_default);
    Ok(match item_type {
        "Weapon" => ItemKind::Weapon(Weapon {
            weapon_type: take_enum(sets, "weapon_type", weapon_type)?.unwrap_or_default(),
            slot: slot(sets)?,
            soulshots: take(sets, "soulshots")?.unwrap_or(0),
            spiritshots: take(sets, "spiritshots")?.unwrap_or(0),
            random_damage: take(sets, "random_damage")?.unwrap_or(0),
            attack_range: take(sets, "attack_range")?.unwrap_or(0),
            damage_range: sets.remove("damage_range").map(damage_range).transpose()?,
            mp_consume: take(sets, "mp_consume")?.unwrap_or(0),
            magic: take(sets, "is_magic_weapon")?.unwrap_or(false),
        }),
        "Armor" => ItemKind::Armor(Armor {
            armor_type: take_enum(sets, "armor_type", armor_type)?.unwrap_or_default(),
            slot: slot(sets)?,
        }),
        "EtcItem" => ItemKind::Etc(EtcItem {
            etc_type: take_enum(sets, "etcitem_type", etc_item_type)?.unwrap_or_default(),
            slot: slot(sets)?,
        }),
        other => return Err(format!("unknown item type `{other}`")),
    })
}

fn stats(node: Node<'_, '_>) -> Result<Vec<StatModifier>> {
    node.children()
        .filter(Node::is_element)
        .map(|modifier| {
            let op = match modifier.tag_name().name() {
                "set" => StatOp::Set,
                "add" => StatOp::Add,
                "sub" => StatOp::Sub,
                "mul" => StatOp::Mul,
                "enchant" => StatOp::Enchant,
                other => return Err(format!("unknown stat operation <{other}>")),
            };
            let name = modifier.attribute("stat").unwrap_or_default();
            Ok(StatModifier {
                stat: stat(name).ok_or_else(|| format!("unknown stat `{name}`"))?,
                op,
                value: parse("val", modifier.attribute("val").unwrap_or_default())?,
                order: modifier.attribute("order").map(|order| parse("order", order)).transpose()?,
            })
        })
        .collect()
}

/// `3599-1;3552-1`
fn skills(list: &str) -> Result<Vec<SkillRef>> {
    list.split(';')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let (id, level) = entry.trim().split_once('-').ok_or_else(|| format!("skill `{entry}` is not id-level"))?;
            Ok(SkillRef { id: SkillId(parse("skill id", id)?), level: parse("skill level", level)? })
        })
        .collect()
}

/// `0;0;40;120`
fn damage_range(text: &str) -> Result<[i32; 4]> {
    let values: Vec<i32> = text.split(';').map(|part| parse("damage_range", part)).collect::<Result<_>>()?;
    values.try_into().map_err(|_| format!("damage_range `{text}` needs four values"))
}

fn take<T: FromStr>(sets: &mut Sets<'_>, key: &str) -> Result<Option<T>> {
    sets.remove(key).map(|value| parse(key, value)).transpose()
}

fn take_enum<T>(sets: &mut Sets<'_>, key: &str, parse: fn(&str) -> Option<T>) -> Result<Option<T>> {
    sets.remove(key).map(|value| parse(value).ok_or_else(|| format!("unknown {key} `{value}`"))).transpose()
}

fn material(name: &str) -> Option<Material> {
    Some(match name {
        "STEEL" => Material::Steel,
        "FINE_STEEL" => Material::FineSteel,
        "BLOOD_STEEL" => Material::BloodSteel,
        "BRONZE" => Material::Bronze,
        "SILVER" => Material::Silver,
        "GOLD" => Material::Gold,
        "MITHRIL" => Material::Mithril,
        "ORIHARUKON" => Material::Oriharukon,
        "DAMASCUS" => Material::Damascus,
        "ADAMANTAITE" => Material::Adamantaite,
        "CHRYSOLITE" => Material::Chrysolite,
        "CRYSTAL" => Material::Crystal,
        "PAPER" => Material::Paper,
        "WOOD" => Material::Wood,
        "CLOTH" => Material::Cloth,
        "COTTON" => Material::Cotton,
        "LEATHER" => Material::Leather,
        "BONE" => Material::Bone,
        "HORN" => Material::Horn,
        "LIQUID" => Material::Liquid,
        "SCALE_OF_DRAGON" => Material::ScaleOfDragon,
        "DYESTUFF" => Material::Dyestuff,
        "COBWEB" => Material::Cobweb,
        "SEED" => Material::Seed,
        "FISH" => Material::Fish,
        "RUNE_XP" => Material::RuneXp,
        "RUNE_SP" => Material::RuneSp,
        "RUNE_REMOVE_PENALTY" => Material::RuneRemovePenalty,
        _ => return None,
    })
}

fn grade(name: &str) -> Option<Grade> {
    Some(match name {
        "NONE" => Grade::None,
        "D" => Grade::D,
        "C" => Grade::C,
        "B" => Grade::B,
        "A" => Grade::A,
        "S" => Grade::S,
        "S80" => Grade::S80,
        "S84" => Grade::S84,
        _ => return None,
    })
}

fn weapon_type(name: &str) -> Option<WeaponType> {
    Some(match name {
        "NONE" => WeaponType::None,
        "SWORD" => WeaponType::Sword,
        "BLUNT" => WeaponType::Blunt,
        "DAGGER" => WeaponType::Dagger,
        "BOW" => WeaponType::Bow,
        "POLE" => WeaponType::Pole,
        "DUAL" => WeaponType::Dual,
        "ETC" => WeaponType::Etc,
        "FIST" => WeaponType::Fist,
        "DUALFIST" => WeaponType::DualFist,
        "FISHINGROD" => WeaponType::FishingRod,
        "RAPIER" => WeaponType::Rapier,
        "ANCIENTSWORD" => WeaponType::AncientSword,
        "CROSSBOW" => WeaponType::Crossbow,
        "DUALDAGGER" => WeaponType::DualDagger,
        "FLAG" => WeaponType::Flag,
        "OWNTHING" => WeaponType::OwnThing,
        _ => return None,
    })
}

fn armor_type(name: &str) -> Option<ArmorType> {
    Some(match name {
        "NONE" => ArmorType::None,
        "LIGHT" => ArmorType::Light,
        "HEAVY" => ArmorType::Heavy,
        "MAGIC" => ArmorType::Magic,
        "SIGIL" => ArmorType::Sigil,
        "SHIELD" => ArmorType::Shield,
        _ => return None,
    })
}

fn slot(name: &str) -> Option<EquipSlot> {
    Some(match name {
        "none" => EquipSlot::None,
        "rhand" => EquipSlot::RightHand,
        "lhand" => EquipSlot::LeftHand,
        "lrhand" => EquipSlot::BothHands,
        "head" => EquipSlot::Head,
        "chest" => EquipSlot::Chest,
        "legs" => EquipSlot::Legs,
        "feet" => EquipSlot::Feet,
        "gloves" => EquipSlot::Gloves,
        "underwear" | "shirt" => EquipSlot::Underwear,
        "back" => EquipSlot::Back,
        "onepiece" | "fullarmor" => EquipSlot::FullArmor,
        "alldress" => EquipSlot::AllDress,
        "neck" => EquipSlot::Neck,
        "rear;lear" => EquipSlot::Ears,
        "rfinger;lfinger" => EquipSlot::Fingers,
        "hair" => EquipSlot::Hair,
        "hair2" => EquipSlot::Face,
        "hairall" | "dhair" => EquipSlot::HairAll,
        "lbracelet" => EquipSlot::LeftBracelet,
        "rbracelet" => EquipSlot::RightBracelet,
        "deco1" | "talisman" => EquipSlot::Talisman,
        "waist" | "belt" => EquipSlot::Belt,
        _ => return None,
    })
}

fn etc_item_type(name: &str) -> Option<EtcItemType> {
    Some(match name {
        "NONE" => EtcItemType::None,
        "ARROW" => EtcItemType::Arrow,
        "BOLT" => EtcItemType::Bolt,
        "POTION" => EtcItemType::Potion,
        "ELIXIR" => EtcItemType::Elixir,
        "SCROLL" => EtcItemType::Scroll,
        "SCRL_ENCHANT_WP" => EtcItemType::EnchantWeapon,
        "SCRL_ENCHANT_AM" => EtcItemType::EnchantArmor,
        "BLESS_SCRL_ENCHANT_WP" => EtcItemType::BlessedEnchantWeapon,
        "BLESS_SCRL_ENCHANT_AM" => EtcItemType::BlessedEnchantArmor,
        "ANCIENT_CRYSTAL_ENCHANT_WP" => EtcItemType::AncientCrystalEnchantWeapon,
        "ANCIENT_CRYSTAL_ENCHANT_AM" => EtcItemType::AncientCrystalEnchantArmor,
        "SCRL_INC_ENCHANT_PROP_WP" => EtcItemType::EnchantChanceWeapon,
        "SCRL_INC_ENCHANT_PROP_AM" => EtcItemType::EnchantChanceArmor,
        "SCRL_ENCHANT_ATTR" => EtcItemType::EnchantAttribute,
        "RECIPE" => EtcItemType::Recipe,
        "MATERIAL" => EtcItemType::Material,
        "PET_COLLAR" => EtcItemType::PetCollar,
        "CASTLE_GUARD" => EtcItemType::CastleGuard,
        "LOTTO" => EtcItemType::Lotto,
        "RACE_TICKET" => EtcItemType::RaceTicket,
        "DYE" => EtcItemType::Dye,
        "SEED" => EtcItemType::Seed,
        "SEED2" => EtcItemType::Seed2,
        "CROP" => EtcItemType::Crop,
        "MATURECROP" => EtcItemType::MatureCrop,
        "HARVEST" => EtcItemType::Harvest,
        "TICKET_OF_LORD" => EtcItemType::TicketOfLord,
        "LURE" => EtcItemType::Lure,
        "COUPON" => EtcItemType::Coupon,
        "RUNE" => EtcItemType::Rune,
        "RUNE_SELECT" => EtcItemType::RuneSelect,
        "SHOT" => EtcItemType::Shot,
        _ => return None,
    })
}

fn action(name: &str) -> Option<ItemAction> {
    Some(match name {
        "NONE" => ItemAction::None,
        "EQUIP" => ItemAction::Equip,
        "CALC" => ItemAction::Calc,
        "CALL_SKILL" => ItemAction::CallSkill,
        "CAPSULE" => ItemAction::Capsule,
        "CREATE_MPCC" => ItemAction::CreateCommandChannel,
        "DICE" => ItemAction::Dice,
        "FISHINGSHOT" => ItemAction::FishingShot,
        "HARVEST" => ItemAction::Harvest,
        "HIDE_NAME" => ItemAction::HideName,
        "KEEP_EXP" => ItemAction::KeepExp,
        "NICK_COLOR" => ItemAction::NickColor,
        "PEEL" => ItemAction::Peel,
        "RECIPE" => ItemAction::Recipe,
        "SEED" => ItemAction::Seed,
        "SHOW_ADVENTURER_GUIDE_BOOK" => ItemAction::ShowAdventurerGuideBook,
        "SHOW_HTML" => ItemAction::ShowHtml,
        "SHOW_SSQ_STATUS" => ItemAction::ShowSevenSignsStatus,
        "SKILL_MAINTAIN" => ItemAction::SkillMaintain,
        "SKILL_REDUCE" => ItemAction::SkillReduce,
        "SOULSHOT" => ItemAction::Soulshot,
        "SPIRITSHOT" => ItemAction::Spiritshot,
        "START_QUEST" => ItemAction::StartQuest,
        "SUMMON_SOULSHOT" => ItemAction::SummonSoulshot,
        "SUMMON_SPIRITSHOT" => ItemAction::SummonSpiritshot,
        "XMAS_OPEN" => ItemAction::ChristmasOpen,
        _ => return None,
    })
}

fn stat(name: &str) -> Option<Stat> {
    Some(match name {
        "maxMp" => Stat::MaxMp,
        "pAtk" => Stat::PAtk,
        "mAtk" => Stat::MAtk,
        "pAtkSpd" => Stat::PAtkSpeed,
        "pDef" => Stat::PDef,
        "mDef" => Stat::MDef,
        "sDef" => Stat::ShieldDef,
        "rShld" => Stat::ShieldRate,
        "rEvas" => Stat::Evasion,
        "critRate" => Stat::CritRate,
        "accCombat" => Stat::Accuracy,
        "pAtkRange" => Stat::AttackRange,
        "fireRes" => Stat::FireRes,
        "waterRes" => Stat::WaterRes,
        "windRes" => Stat::WindRes,
        "earthRes" => Stat::EarthRes,
        "holyRes" => Stat::HolyRes,
        "darkRes" => Stat::DarkRes,
        "holyPower" => Stat::HolyPower,
        "magicSuccRes" => Stat::MagicSuccessRes,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT_SWORD: &str = r#"<list>
        <item id="1" type="Weapon" name="Short Sword">
            <set name="icon" val="icon.weapon_small_sword_i00" />
            <set name="default_action" val="EQUIP" />
            <set name="weapon_type" val="SWORD" />
            <set name="bodypart" val="rhand" />
            <set name="damage_range" val="0;0;40;120" />
            <set name="material" val="STEEL" />
            <set name="weight" val="1600" />
            <set name="price" val="590" />
            <set name="is_tradable" val="false" />
            <set name="item_skill" val="3599-1;3552-2" />
            <set name="change_weaponId" val="7" />
            <stats><set stat="pAtk" val="8" /><enchant stat="pAtk" val="4" order="16" /></stats>
            <conditions><player level="10" /></conditions>
        </item>
        <item id="2" type="Weapon" name="Broken"><set name="weapon_type" val="LASER" /></item>
    </list>"#;

    #[test]
    fn maps_gameplay_and_reports_the_rest() {
        let document = roxmltree::Document::parse(SHORT_SWORD).unwrap();
        let nodes: Vec<_> = document.root_element().children().filter(|node| node.has_tag_name("item")).collect();
        let mut report = Report::default();
        let (_, item) = item(nodes[0], &mut report).unwrap();

        assert_eq!(item.visual.icon.as_ref().map(TextureRef::path), Some("icon.weapon_small_sword_i00"));
        assert_eq!((item.weight, item.price, item.action), (1600, 590, ItemAction::Equip));
        assert!(!item.flags.tradable && item.flags.sellable);
        assert_eq!(item.skills[1], SkillRef { id: SkillId(3552), level: 2 });
        assert_eq!(item.stats[1], StatModifier { stat: Stat::PAtk, op: StatOp::Enchant, value: 4.0, order: Some(16) });
        let ItemKind::Weapon(weapon) = &item.kind else { panic!("{:?}", item.kind) };
        assert_eq!(
            (weapon.weapon_type, weapon.slot, weapon.damage_range),
            (WeaponType::Sword, EquipSlot::RightHand, Some([0, 0, 40, 120]))
        );
        assert_eq!(report.unmodeled.keys().collect::<Vec<_>>(), ["<conditions>", "set:change_weaponId on Weapon"]);

        let Err(error) = super::item(nodes[1], &mut report) else { panic!("LASER is not a weapon type") };
        assert!(error.contains("unknown weapon_type `LASER`"), "{error}");
    }
}
