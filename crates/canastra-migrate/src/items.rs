//! Items: server gameplay merged with client presentation and names.

mod client;
mod server;

use std::collections::BTreeMap;

use canastra_data::id::ItemId;
use canastra_data::item::{Item, ItemKind};
use canastra_data::text::Locale;

use crate::{Report, Severity, Sources, index, server_index};
use client::{ClientItem, ClientName, ClientTable};

pub(crate) fn migrate(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<ItemId, Item> {
    let mut items = server_index(sources.server_items, "item", report, server::item);

    let visuals = client_items(sources, report);
    let id = |name: &ClientName| name.id;
    let names = index("ItemName", sources.item_names, client::name, id, Severity::Error, report);

    for (&id, item) in &mut items {
        match visuals.get(&id) {
            Some(client) => apply_visual(item, client, report),
            None => report.push(Severity::Warning, subject(id), "no client presentation"),
        }
        if let Some(name) = names.get(&id) {
            if name.name.get(Locale::En).is_some() {
                item.name = name.name.clone();
            }
            item.additional_name = name.additional_name.clone();
            item.description = name.description.clone();
        }
    }
    for id in visuals.keys().filter(|id| !items.contains_key(id)) {
        report.push(Severity::Warning, subject(*id), "only in the client; not migrated");
    }
    items
}

fn client_items(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<ItemId, ClientItem> {
    let id = |item: &ClientItem| item.id;
    let mut visuals = index("Weapongrp", sources.weapons, client::weapon, id, Severity::Error, report);
    for (table, records, map) in [
        ("Armorgrp", sources.armors, client::armor as fn(&_) -> _),
        ("EtcItemgrp", sources.etc_items, client::etc_item),
    ] {
        for (item_id, item) in index(table, records, map, id, Severity::Error, report) {
            if visuals.insert(item_id, item).is_some() {
                report.push(Severity::Error, subject(item_id), format!("{table}: already defined by another table"));
            }
        }
    }
    visuals
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
