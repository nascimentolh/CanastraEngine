//! Player classes from the reference server: the class tree (`classList.xml`), one template per class
//! (`baseStats/*.xml`) and initial equipment (`initialEquipment.xml`).
//!
//! The legacy files repeat a starting class's static data in every class of its line; the model keeps it
//! once, on the starting class, and reports any class whose copy differs. Advanced classes keep only their
//! parent and their own level gains.

mod equipment;
mod template;

use std::collections::BTreeMap;

use canastra_data::class::{Archetype, Origin, PlayerClass, StartingClass};
use canastra_data::id::ClassId;
use canastra_data::npc::{Race, Sex};
use canastra_data::text::Localized;

use l2_dat::Value;

use crate::fields::{Fields, Result, parse};
use crate::{Report, Severity, Sources};

pub(crate) fn migrate(sources: &Sources<'_>, report: &mut Report) -> BTreeMap<ClassId, PlayerClass> {
    let tree = match class_tree(sources.server_class_list) {
        Ok(tree) => tree,
        Err(error) => {
            report.push(Severity::Error, "classList.xml", error);
            return BTreeMap::new();
        }
    };
    let mut templates = BTreeMap::new();
    for (file, xml) in sources.server_class_templates {
        match template::parse_document(xml, report) {
            Ok(parsed) => {
                templates.insert(parsed.class, parsed);
            }
            Err(error) => report.push(Severity::Error, file.clone(), error),
        }
    }
    let mut equipment = equipment::parse_document(sources.server_initial_equipment).unwrap_or_else(|error| {
        report.push(Severity::Error, "initialEquipment.xml", error);
        BTreeMap::new()
    });

    let mut descriptions = descriptions(sources.class_info, report);
    let mut classes = BTreeMap::new();
    for (&id, (name, parent)) in &tree {
        let subject = format!("class {}", id.0);
        let Some(parsed) = templates.get(&id) else {
            report.push(Severity::Error, subject, "no template; class skipped");
            continue;
        };
        let origin = if let Some(parent) = parent {
            let start = starting_ancestor(&tree, id).and_then(|start| templates.get(&start));
            if start.is_some_and(|start| {
                start.template != parsed.template || start.creation_points != parsed.creation_points
            }) {
                let message = "static data differs from its starting class; starting class kept";
                report.push(Severity::Warning, subject.clone(), message);
            }
            Origin::Advanced { parent: *parent }
        } else {
            let Some((race, archetype, sex)) = line(id) else {
                report.push(Severity::Error, subject, "a starting class with no known race; class skipped");
                continue;
            };
            Origin::Starting(Box::new(StartingClass {
                race,
                archetype,
                sex,
                description: descriptions.remove(&id).unwrap_or_default(),
                template: parsed.template,
                creation_points: parsed.creation_points.clone(),
                initial_items: equipment.remove(&id).unwrap_or_default(),
            }))
        };
        classes.insert(id, PlayerClass { id, name: Localized::en(name), levels: parsed.levels.clone(), origin });
    }
    for class in equipment.keys() {
        report.push(
            Severity::Warning,
            format!("class {}", class.0),
            "initial equipment for a class nobody starts as; dropped",
        );
    }
    classes
}

/// The starting classes in the order the client's creation screen lists them, which `ClassInfo` numbers from 1.
const CREATION_ORDER: [u16; 11] = [0, 10, 18, 25, 31, 38, 44, 49, 53, 123, 124];

/// Each starting class's creation screen text from `ClassInfo` records; number 0 is the screen's own prompt.
fn descriptions(records: &[Value], report: &mut Report) -> BTreeMap<ClassId, Localized> {
    let mut descriptions = BTreeMap::new();
    for record in records {
        let described =
            Fields::of(record).and_then(|mut fields| Ok((fields.uint("class")?, fields.text("description")?)));
        match described {
            Ok((0, _)) => {}
            Ok((number, text)) => {
                let class = usize::try_from(number).ok().and_then(|number| CREATION_ORDER.get(number - 1));
                if let Some(&class) = class {
                    descriptions.insert(ClassId(class), Localized::en(text.replace("\r\n", "\n")));
                } else {
                    report.push(Severity::Warning, "ClassInfo-e", format!("no starting class numbered {number}"));
                }
            }
            Err(error) => report.push(Severity::Error, "ClassInfo-e", error),
        }
    }
    descriptions
}

/// Each class's name and parent.
fn class_tree(xml: &str) -> Result<BTreeMap<ClassId, (String, Option<ClassId>)>> {
    let document = roxmltree::Document::parse(xml).map_err(|error| error.to_string())?;
    document
        .root_element()
        .children()
        .filter(|node| node.has_tag_name("class"))
        .map(|node| {
            let id = ClassId(parse("classId", node.attribute("classId").unwrap_or_default())?);
            let parent = node
                .attribute("parentClassId")
                .map(|parent| parse("parentClassId", parent).map(ClassId))
                .transpose()?;
            Ok((id, (node.attribute("name").unwrap_or_default().to_owned(), parent)))
        })
        .collect()
}

/// The starting class `id` descends from, following parents.
fn starting_ancestor(tree: &BTreeMap<ClassId, (String, Option<ClassId>)>, mut id: ClassId) -> Option<ClassId> {
    // A class tree is shallow; the bound only guards against a cycle in bad data.
    for _ in 0..tree.len() {
        match tree.get(&id)? {
            (_, Some(parent)) => id = *parent,
            (_, None) => return Some(id),
        }
    }
    None
}

/// Race, archetype and fixed sex of each starting class. The reference server keeps these in code, not data.
fn line(id: ClassId) -> Option<(Race, Archetype, Option<Sex>)> {
    use Archetype::{Fighter, Mystic};
    Some(match id.0 {
        0 => (Race::Human, Fighter, None),
        10 => (Race::Human, Mystic, None),
        18 => (Race::Elf, Fighter, None),
        25 => (Race::Elf, Mystic, None),
        31 => (Race::DarkElf, Fighter, None),
        38 => (Race::DarkElf, Mystic, None),
        44 => (Race::Orc, Fighter, None),
        49 => (Race::Orc, Mystic, None),
        53 => (Race::Dwarf, Fighter, None),
        123 => (Race::Kamael, Fighter, Some(Sex::Male)),
        124 => (Race::Kamael, Fighter, Some(Sex::Female)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
