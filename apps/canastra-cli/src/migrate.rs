//! `canastra migrate`: converts legacy client tables and server XML into game data.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use canastra_data::class::Origin;
use canastra_data::format;
use canastra_data::item::{ItemKind, ItemModel};
use canastra_migrate::{Report, Severity, Sources};
use l2_dat::Value;

use crate::{Result, read_decrypted, records_of};

/// How many examples to print per kind of diagnostic.
const EXAMPLES: usize = 3;

/// `server_stats` holds the reference server's `items`, `skills` and `npcs` XML folders, `chars/classList.xml`,
/// `chars/baseStats` and `initialEquipment.xml`.
pub(crate) fn run(client_root: &Path, server_stats: &Path, output: Option<&Path>) -> Result {
    let table = |file: &str| -> Result<Vec<Value>> {
        let (_, plain) = read_decrypted(&client_root.join("system").join(file))?;
        let layout = l2_dat_h5::table(file).ok_or_else(|| format!("no H5 layout for {file}"))?;
        Ok(records_of(&l2_dat::decode(layout, &plain)?).to_vec())
    };
    let (weapons, armors, etc_items, item_names) =
        (table("Weapongrp.dat")?, table("Armorgrp.dat")?, table("EtcItemgrp.dat")?, table("ItemName-e.dat")?);
    let (skills, skill_names, skill_sounds) =
        (table("Skillgrp.dat")?, table("SkillName-e.dat")?, table("SkillSoundgrp.dat")?);
    let (npcs, npc_names) = (table("Npcgrp.dat")?, table("NpcName-e.dat")?);
    let (server_items, server_skills, server_npcs) = (
        xml_documents(&server_stats.join("items"))?,
        xml_documents(&server_stats.join("skills"))?,
        xml_documents(&server_stats.join("npcs"))?,
    );
    let read = |path: &Path| fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()));
    let class_list = read(&server_stats.join("chars").join("classList.xml"))?;
    let class_templates = xml_documents(&server_stats.join("chars").join("baseStats"))?;
    let initial_equipment = read(&server_stats.join("initialEquipment.xml"))?;

    let (data, report) = canastra_migrate::migrate(&Sources {
        weapons: &weapons,
        armors: &armors,
        etc_items: &etc_items,
        item_names: &item_names,
        skills: &skills,
        skill_names: &skill_names,
        skill_sounds: &skill_sounds,
        npcs: &npcs,
        npc_names: &npc_names,
        server_items: &server_items,
        server_skills: &server_skills,
        server_npcs: &server_npcs,
        server_class_list: &class_list,
        server_class_templates: &class_templates,
        server_initial_equipment: &initial_equipment,
    });
    let starting = data.classes.values().filter(|class| matches!(class.origin, Origin::Starting(_))).count();
    println!("{} classes, {starting} to start as", data.classes.len());

    let items = &data.items;
    let count = |kind: fn(&ItemKind) -> bool| items.values().filter(|item| kind(&item.kind)).count();
    let modeled = items.values().filter(|item| item.visual.model != ItemModel::None).count();
    println!(
        "{} items ({} weapons, {} armor, {} etc), {modeled} with an equip model",
        items.len(),
        count(|kind| matches!(kind, ItemKind::Weapon(_))),
        count(|kind| matches!(kind, ItemKind::Armor(_))),
        count(|kind| matches!(kind, ItemKind::Etc(_))),
    );
    let levels: usize = data.skills.values().map(|skill| skill.levels.len()).sum();
    println!("{} skills ({levels} levels), {} npcs", data.skills.len(), data.npcs.len());
    print_report(&report);

    let issues = data.validate();
    println!("validation: {} issues", issues.len());
    for issue in issues.iter().take(EXAMPLES) {
        println!("  {:?}: {:?}", issue.subject, issue.problem);
    }
    let errors = report.count(Severity::Error);
    if errors > 0 {
        return Err(format!("{errors} migration error(s); nothing written").into());
    }
    if let Some(output) = output {
        fs::write(output, format::encode(&data))?;
        // Read the file back so what landed on disk is what was migrated.
        let written = fs::read(output)?;
        if format::decode(&written)? != data {
            return Err(format!("{} does not read back as the migrated data", output.display()).into());
        }
        println!("wrote {} ({} bytes, verified)", output.display(), written.len());
    }
    Ok(())
}

fn xml_documents(dir: &Path) -> Result<Vec<(String, String)>> {
    let mut documents = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("xml")) {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            documents.push((name, fs::read_to_string(&path)?));
        }
    }
    documents.sort();
    Ok(documents)
}

fn print_report(report: &Report) {
    for severity in [Severity::Error, Severity::Warning] {
        // Group by message shape so thousands of similar lines read as one row.
        let mut groups: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
        for diagnostic in report.diagnostics.iter().filter(|d| d.severity == severity) {
            let shape: String = diagnostic.message.chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
            groups.entry(shape).or_default().push((&diagnostic.subject, &diagnostic.message));
        }
        println!("{severity:?}s: {}", report.count(severity));
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by_key(|(_, entries)| std::cmp::Reverse(entries.len()));
        for (shape, entries) in groups {
            println!("  {:>6}  {shape}", entries.len());
            for (subject, message) in entries.iter().take(EXAMPLES) {
                println!("          {subject}: {message}");
            }
        }
    }
    println!("unmodeled server data: {}", report.unmodeled.len());
    let mut unmodeled: Vec<_> = report.unmodeled.iter().collect();
    unmodeled.sort_by_key(|&(_, count)| std::cmp::Reverse(*count));
    for (key, count) in unmodeled {
        println!("  {count:>6}  {key}");
    }
}
