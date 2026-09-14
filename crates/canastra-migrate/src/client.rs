//! Item presentation and names from the H5 client tables.

use canastra_data::asset::AssetRef;
use canastra_data::id::ItemId;
use canastra_data::item::{Attachment, Body, BodyModel, HeldModel, ItemModel, ItemVisual, ModelPart, WornModel};
use canastra_data::text::Localized;
use l2_dat::Value;

use crate::fields::{Fields, Result};

/// Client copies of gameplay values; the server definition is authoritative.
const GAMEPLAY_COPIES: &[&str] = &[
    "durability",
    "weight",
    "material_type",
    "crystallizable",
    "crystal_type",
    "body_part",
    "random_damage",
    "patt",
    "matt",
    "weapon_type",
    "critical",
    "hit_mod",
    "avoid_mod",
    "shield_pdef",
    "shield_rate",
    "speed",
    "mp_consume",
    "soulshot_count",
    "spiritshot_count",
    "can_equip_hero",
    "armor_type",
    "pdef",
    "mdef",
    "mpbonus",
    "consume_type",
    "etcitem_type",
];

/// Values whose meaning is not known; nothing in the engine reads them yet.
/// The migration can be rerun against the client once one is understood.
const UNKNOWN: &[&str] = &[
    "tag",
    "drop_type",
    "drop_anim_type",
    "color",
    "UNK_0",
    "UNK_1",
    "UNK_6",
    "UNK_7",
    "UNK_8",
    "UNK_9",
    "UNK_10",
    "UNK_12",
    "newdata",
    "newdata1",
    "newdata2",
    "newdata3",
    "newdata4",
    "newdata5",
    "newdata6",
    "newdata7",
    "newdata8",
    "curvature",
    "effA",
    "effB",
    "rangeA",
    "rangeB",
    "junk",
    "junk1A",
    "junk2A",
    "junk1B1",
    "junk1B2",
    "junk1B3",
    "junk1B4",
    "junk1B5",
    "junk2B1",
    "junk2B2",
    "junk2B3",
    "junk2B4",
    "junk2B5",
    "junk2B6",
    "variation_icon",
    "popup",
];

/// Data for systems that are not modeled yet (quests, item sets, special enchants).
const NOT_MODELED_YET: &[&str] = &[
    "related_quest_id",
    "supercnt0",
    "setid_1",
    "set_bonus_desc",
    "supercnt1",
    "set_extra_id",
    "set_extra_desc",
    "unknown",
    "special_enchant_amount",
    "special_enchant_desc",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientTable {
    Weapon,
    Armor,
    Etc,
}

pub(crate) struct ClientItem {
    pub(crate) id: ItemId,
    pub(crate) table: ClientTable,
    pub(crate) visual: ItemVisual,
    /// Data that was dropped or repaired while mapping.
    pub(crate) notes: Vec<String>,
}

pub(crate) struct ClientName {
    pub(crate) id: ItemId,
    pub(crate) name: Localized,
    pub(crate) additional_name: Localized,
    pub(crate) description: Localized,
}

/// Collects references and notes what had to be dropped or repaired.
#[derive(Default)]
struct Assets {
    notes: Vec<String>,
}

impl Assets {
    fn one<K>(&mut self, path: &str) -> Option<AssetRef<K>> {
        if path.is_empty() || path.eq_ignore_ascii_case("[none]") {
            return None;
        }
        AssetRef::parse(path).map_err(|error| self.notes.push(format!("dropped asset reference: {error}"))).ok()
    }

    fn many<K>(&mut self, paths: &[&str]) -> Vec<AssetRef<K>> {
        paths.iter().filter_map(|path| self.one(path)).collect()
    }
}

pub(crate) fn weapon(value: &Value) -> Result<ClientItem> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let id = ItemId(f.uint("object_id")?);
    let mut visual = common(&mut f, &mut assets)?;
    visual.drop.meshes = assets.many(&group_texts(&mut f, "drop_mesh")?);
    visual.drop.textures = assets.many(&group_texts(&mut f, "drop_texture")?);
    visual.sounds = assets.many(&f.texts("item_sound")?);

    let mut meshes = f.nested("wp_mesh")?;
    let mesh_paths = meshes.texts("mesh_0_")?;
    let texture_counts = meshes.uints("mesh_1_")?;
    meshes.finish()?;
    let textures = f.texts("texture")?;
    let parts = held_parts(&mesh_paths, Some(&texture_counts), &textures, &mut assets);

    let grip = f.uint("handness")?;
    let effect = assets.one(f.text("effect")?);
    visual.model = ItemModel::Held(HeldModel { parts, effect, grip });
    finish(f, id, ClientTable::Weapon, visual, assets)
}

pub(crate) fn armor(value: &Value) -> Result<ClientItem> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let id = ItemId(f.uint("object_id")?);
    let mut visual = common(&mut f, &mut assets)?;
    // A one-element list holding the drop meshes and their textures.
    for drop in f.list("drop_texture")? {
        let mut drop = Fields::of(drop)?;
        let meshes = [drop.text("drop_mesh1")?, drop.text("drop_mesh2")?, drop.text("drop_mesh3")?];
        visual.drop.meshes.extend(assets.many(&meshes));
        visual.drop.textures.extend(assets.many(&drop.texts("drop_texture")?));
        drop.finish()?;
    }
    visual.sounds = assets.many(&f.texts("item_sound")?);

    let mut worn = WornModel::default();
    for (name, body) in BODIES {
        let mut main = f.nested(name)?;
        let mut model = body_model(&mut main, &mut assets)?;
        main.finish()?;
        let mut extra = f.nested(&format!("{name}_add"))?;
        attachments(&mut extra, &mut model, &mut assets)?;
        extra.finish()?;
        if model != BodyModel::default() {
            worn.bodies.insert(body, model);
        }
    }
    worn.hit_effect = assets.one(f.text("attack_effect")?);
    visual.model = ItemModel::Worn(worn);
    finish(f, id, ClientTable::Armor, visual, assets)
}

pub(crate) fn etc_item(value: &Value) -> Result<ClientItem> {
    let mut f = Fields::of(value)?;
    let mut assets = Assets::default();
    let id = ItemId(f.uint("object_id")?);
    let mut visual = common(&mut f, &mut assets)?;
    visual.drop.meshes = assets.many(&group_texts(&mut f, "drop_mesh")?);
    visual.drop.textures = assets.many(&group_texts(&mut f, "drop_texture")?);

    let meshes: Vec<_> = f.texts("mesh")?.into_iter().filter(|path| !path.is_empty()).collect();
    let textures = f.texts("texture")?;
    if !meshes.is_empty() {
        let parts = held_parts(&meshes, None, &textures, &mut assets);
        visual.model = ItemModel::Held(HeldModel { parts, effect: None, grip: 0 });
    }
    finish(f, id, ClientTable::Etc, visual, assets)
}

pub(crate) fn name(value: &Value) -> Result<ClientName> {
    let mut f = Fields::of(value)?;
    let localized = |text: &str| if text.is_empty() { Localized::default() } else { Localized::en(text) };
    let name = ClientName {
        id: ItemId(f.uint("id")?),
        name: localized(f.text("name")?),
        additional_name: localized(f.text("additionalname")?),
        description: localized(f.text("description")?),
    };
    f.ignore(UNKNOWN);
    f.ignore(NOT_MODELED_YET);
    f.finish()?;
    Ok(name)
}

/// Fields shared by the weapon, armor and etc tables.
fn common(f: &mut Fields<'_>, assets: &mut Assets) -> Result<ItemVisual> {
    let mut visual = ItemVisual::default();
    let icons = group_texts(f, "icon")?;
    let mut icons = assets.many(&icons).into_iter();
    visual.icon = icons.next();
    visual.part_icons = icons.collect();
    visual.icon_panel = assets.one(f.text("icon_panel")?);
    visual.drop.radius = f.uint("drop_radius")?;
    visual.drop.height = f.uint("drop_height")?;
    visual.drop.sound = assets.one(f.text("drop_sound")?);
    visual.equip_sound = assets.one(f.text("equip_sound")?);
    f.ignore(GAMEPLAY_COPIES);
    f.ignore(UNKNOWN);
    f.ignore(NOT_MODELED_YET);
    Ok(visual)
}

fn finish(f: Fields<'_>, id: ItemId, table: ClientTable, visual: ItemVisual, assets: Assets) -> Result<ClientItem> {
    f.finish()?;
    Ok(ClientItem { id, table, visual, notes: assets.notes })
}

/// Every text field of a group, in order.
fn group_texts<'a>(f: &mut Fields<'a>, name: &str) -> Result<Vec<&'a str>> {
    f.nested(name)?.into_texts()
}

/// Assigns textures to meshes. A single mesh owns every texture; several meshes split the
/// list by `counts` (one texture each when absent). When the numbers disagree, textures are
/// assigned in order, the last mesh takes the rest, and the repair is noted.
fn held_parts(meshes: &[&str], counts: Option<&[u32]>, textures: &[&str], assets: &mut Assets) -> Vec<ModelPart> {
    if let [mesh] = meshes {
        return vec![ModelPart { mesh: assets.one(mesh), textures: assets.many(textures) }];
    }
    let counts: Vec<usize> = counts.map_or_else(|| vec![1; meshes.len()], |c| c.iter().map(|&n| n as usize).collect());
    if counts.len() != meshes.len() || counts.iter().sum::<usize>() != textures.len() {
        assets.notes.push(format!(
            "{} meshes, texture counts {counts:?} and {} textures disagree; assigned in order",
            meshes.len(),
            textures.len()
        ));
    }
    let mut rest = textures;
    let mut parts = Vec::with_capacity(meshes.len());
    for (index, mesh) in meshes.iter().enumerate() {
        let count = if index + 1 == meshes.len() { rest.len() } else { counts.get(index).copied().unwrap_or(0) };
        let (own, tail) = rest.split_at_checked(count).unwrap_or((rest, &[]));
        parts.push(ModelPart { mesh: assets.one(mesh), textures: assets.many(own) });
        rest = tail;
    }
    parts
}

const BODIES: [(&str, Body); 17] = [
    ("m_HumnFigh", Body::HumanFighterMale),
    ("f_HumnFigh", Body::HumanFighterFemale),
    ("m_HumnMyst", Body::HumanMysticMale),
    ("f_HumnMyst", Body::HumanMysticFemale),
    ("m_Elf", Body::ElfMale),
    ("f_Elf", Body::ElfFemale),
    ("m_DarkElf", Body::DarkElfMale),
    ("f_DarkElf", Body::DarkElfFemale),
    ("m_OrcFigh", Body::OrcFighterMale),
    ("f_OrcFigh", Body::OrcFighterFemale),
    ("m_OrcMage", Body::OrcMysticMale),
    ("f_OrcMage", Body::OrcMysticFemale),
    ("m_Dorf", Body::DwarfMale),
    ("f_Dorf", Body::DwarfFemale),
    ("m_Kamael", Body::KamaelMale),
    ("f_Kamael", Body::KamaelFemale),
    ("NPC", Body::Npc),
];

fn body_model(f: &mut Fields<'_>, assets: &mut Assets) -> Result<BodyModel> {
    let model = BodyModel {
        meshes: assets.many(&f.texts("meshes")?),
        textures: assets.many(&f.texts("textures")?),
        ..BodyModel::default()
    };
    Ok(model)
}

fn attachments(f: &mut Fields<'_>, model: &mut BodyModel, assets: &mut Assets) -> Result<()> {
    for entry in f.list("meshes")? {
        let mut entry = Fields::of(entry)?;
        let mesh = assets.one(entry.text("mesh")?);
        let params = [entry.int8("val1")?, entry.int8("val2")?];
        entry.finish()?;
        if let Some(mesh) = mesh {
            model.attachments.push(Attachment { mesh, params });
        }
    }
    model.attachment_textures = assets.many(&f.texts("textures")?);
    model.extra_texture = assets.one(f.text("extra_texture")?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part_textures(parts: &[ModelPart]) -> Vec<Vec<&str>> {
        parts.iter().map(|part| part.textures.iter().map(AssetRef::path).collect()).collect()
    }

    #[test]
    fn assigns_textures_to_meshes() {
        let mut assets = Assets::default();
        let single = held_parts(&["W.shield"], Some(&[2]), &["T.a"], &mut assets);
        assert_eq!(part_textures(&single), [vec!["T.a"]]);

        let dual = held_parts(&["W.a", "W.b"], Some(&[2, 1]), &["T.a0", "T.a1", "T.b0"], &mut assets);
        assert_eq!(part_textures(&dual), [vec!["T.a0", "T.a1"], vec!["T.b0"]]);
        assert!(assets.notes.is_empty());

        let pair = held_parts(&["W.a", "W.b"], None, &["T.a", "T.b"], &mut assets);
        assert_eq!(part_textures(&pair), [vec!["T.a"], vec!["T.b"]]);

        let broken = held_parts(&["W.a", "W.b"], Some(&[3, 1]), &["T.a", "T.b"], &mut assets);
        assert_eq!(part_textures(&broken), [vec!["T.a", "T.b"], vec![]]);
        assert_eq!(assets.notes.len(), 1);
    }
}
