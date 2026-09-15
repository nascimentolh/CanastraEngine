//! `initialEquipment.xml`: what new characters of each class carry, including items that only last a
//! while (`minutes`, the recruit kit).

use std::collections::BTreeMap;

use canastra_data::class::InitialItem;
use canastra_data::id::{ClassId, ItemId};

use crate::fields::{Result, parse};

pub(super) fn parse_document(xml: &str) -> Result<BTreeMap<ClassId, Vec<InitialItem>>> {
    let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
    let mut equipment = BTreeMap::new();
    for list in document.root_element().children().filter(|node| node.has_tag_name("equipment")) {
        let class = ClassId(parse("classId", list.attribute("classId").unwrap_or_default())?);
        let items = list
            .children()
            .filter(|node| node.has_tag_name("item"))
            .map(|item| {
                let minutes: u32 = item.attribute("minutes").map_or(Ok(0), |minutes| parse("minutes", minutes))?;
                Ok(InitialItem {
                    item: ItemId(parse("id", item.attribute("id").unwrap_or_default())?),
                    count: parse("count", item.attribute("count").unwrap_or_default())?,
                    equipped: item.attribute("equipped").map_or(Ok(false), |equipped| parse("equipped", equipped))?,
                    // Zero, like a missing attribute, means the item is permanent.
                    lasts_minutes: (minutes > 0).then_some(minutes),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        equipment.insert(class, items);
    }
    Ok(equipment)
}
