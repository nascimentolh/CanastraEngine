//! `canastra migrate`: converts legacy client tables and server XML into game data.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use canastra_data::GameData;
use canastra_data::item::{ItemKind, ItemModel};
use canastra_migrate::{ItemSources, Report, Severity};
use l2_dat::Value;

use crate::{Result, read_decrypted, records_of};

/// How many examples to print per kind of diagnostic.
const EXAMPLES: usize = 3;

pub(crate) fn run(client_root: &Path, server_items: &Path) -> Result {
    let table = |file: &str| -> Result<Vec<Value>> {
        let (_, plain) = read_decrypted(&client_root.join("system").join(file))?;
        let layout = l2_dat_h5::table(file).ok_or_else(|| format!("no H5 layout for {file}"))?;
        Ok(records_of(&l2_dat::decode(layout, &plain)?).to_vec())
    };
    let (weapons, armors, etc_items, names) =
        (table("Weapongrp.dat")?, table("Armorgrp.dat")?, table("EtcItemgrp.dat")?, table("ItemName-e.dat")?);
    let server_documents = xml_documents(server_items)?;

    let (items, report) = canastra_migrate::migrate_items(&ItemSources {
        weapons: &weapons,
        armors: &armors,
        etc_items: &etc_items,
        names: &names,
        server_documents: &server_documents,
    });

    let count = |kind: fn(&ItemKind) -> bool| items.values().filter(|item| kind(&item.kind)).count();
    let weapons_count = count(|kind| matches!(kind, ItemKind::Weapon(_)));
    let armor_count = count(|kind| matches!(kind, ItemKind::Armor(_)));
    let etc_count = count(|kind| matches!(kind, ItemKind::Etc(_)));
    let modeled = items.values().filter(|item| item.visual.model != ItemModel::None).count();
    let with_icon = items.values().filter(|item| item.visual.icon.is_some()).count();
    println!(
        "{} items ({} weapons, {} armor, {} etc); {with_icon} with icon, {modeled} with an equip model",
        items.len(),
        weapons_count,
        armor_count,
        etc_count
    );
    print_report(&report);

    let issues = GameData { items }.validate();
    println!("validation: {} issues", issues.len());
    for issue in issues.iter().take(EXAMPLES) {
        println!("  item {}: {:?}", issue.item.0, issue.problem);
    }
    match report.count(Severity::Error) {
        0 => Ok(()),
        n => Err(format!("{n} migration error(s)").into()),
    }
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
