//! Items: server gameplay merged with client presentation and names.

use std::collections::BTreeMap;

use canastra_data::id::ItemId;
use canastra_data::item::{Item, ItemKind};
use l2_dat::Value;

use crate::client::{self, ClientItem, ClientName, ClientTable};
use crate::{Report, Severity, server};

/// Decoded legacy inputs. Client lists are the records of each table.
pub struct ItemSources<'a> {
    pub weapons: &'a [Value],
    pub armors: &'a [Value],
    pub etc_items: &'a [Value],
    pub names: &'a [Value],
    /// `(file name, XML text)` for every server item document.
    pub server_documents: &'a [(String, String)],
}

/// The server decides what exists and how it plays; the client supplies names
/// and presentation. Client-only items are not playable and are left out.
pub fn migrate_items(sources: &ItemSources<'_>) -> (BTreeMap<ItemId, Item>, Report) {
    let mut report = Report::default();
    let mut items = BTreeMap::new();
    for (file, xml) in sources.server_documents {
        match server::parse(xml, file, &mut report) {
            Ok(parsed) => {
                for mut server_item in parsed {
                    let id = server_item.item.id;
                    server_item.item.visual.icon = server_item.icon;
                    if items.insert(id, server_item.item).is_some() {
                        report.push(Severity::Error, subject(id), format!("{file}: defined more than once"));
                    }
                }
            }
            Err(error) => report.push(Severity::Error, file.clone(), error),
        }
    }

    let visuals = client_items(sources, &mut report);
    let names = client_records("ItemName", sources.names, client::name, |name: &ClientName| name.id, &mut report);

    for (&id, item) in &mut items {
        match visuals.get(&id) {
            Some(client) => apply_visual(item, client, &mut report),
            None => report.push(Severity::Warning, subject(id), "no client presentation"),
        }
        if let Some(name) = names.get(&id) {
            if name.name.get(canastra_data::text::Locale::En).is_some() {
                item.name = name.name.clone();
            }
            item.additional_name = name.additional_name.clone();
            item.description = name.description.clone();
        }
    }
    for id in visuals.keys().filter(|id| !items.contains_key(id)) {
        report.push(Severity::Warning, subject(*id), "only in the client; not migrated");
    }
    (items, report)
}

fn client_items(sources: &ItemSources<'_>, report: &mut Report) -> BTreeMap<ItemId, ClientItem> {
    let id = |item: &ClientItem| item.id;
    let mut visuals = client_records("Weapongrp", sources.weapons, client::weapon, id, report);
    for (table, records, map) in [
        ("Armorgrp", sources.armors, client::armor as fn(&Value) -> _),
        ("EtcItemgrp", sources.etc_items, client::etc_item),
    ] {
        for (item_id, item) in client_records(table, records, map, id, report) {
            if visuals.insert(item_id, item).is_some() {
                report.push(Severity::Error, subject(item_id), format!("{table}: already defined by another table"));
            }
        }
    }
    visuals
}

fn client_records<T>(
    table: &str,
    records: &[Value],
    map: fn(&Value) -> crate::fields::Result<T>,
    id: fn(&T) -> ItemId,
    report: &mut Report,
) -> BTreeMap<ItemId, T> {
    let mut out = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        match map(record) {
            Ok(entry) => {
                let entry_id = id(&entry);
                if out.insert(entry_id, entry).is_some() {
                    report.push(Severity::Error, subject(entry_id), format!("{table}: duplicate record"));
                }
            }
            Err(error) => report.push(Severity::Error, format!("{table} record {index}"), error),
        }
    }
    out
}

fn apply_visual(item: &mut Item, client: &ClientItem, report: &mut Report) {
    let server_icon = item.visual.icon.take();
    item.visual = client.visual.clone();
    if item.visual.icon.is_none() {
        item.visual.icon = server_icon;
    }
    for note in &client.notes {
        report.push(Severity::Warning, subject(item.id), note.clone());
    }
    let drawn_as = match (&item.kind, client.table) {
        (ItemKind::Weapon(_), ClientTable::Weapon)
        | (ItemKind::Armor(_), ClientTable::Armor)
        | (ItemKind::Etc(_), ClientTable::Etc) => return,
        (_, ClientTable::Weapon) => "a weapon",
        (_, ClientTable::Armor) => "armor",
        (_, ClientTable::Etc) => "an etc item",
    };
    report.push(Severity::Warning, subject(item.id), format!("client draws it as {drawn_as}; server kind wins"));
}

fn subject(id: ItemId) -> String {
    format!("item {}", id.0)
}
