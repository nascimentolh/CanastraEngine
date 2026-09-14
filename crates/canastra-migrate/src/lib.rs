//! Converts legacy Lineage 2 sources into Canastra game data.
//!
//! Inputs are already decoded: client tables from `l2-dat` and server XML as text.
//! The server decides what exists and how it plays; the client supplies names and
//! presentation. Every legacy client field is either mapped or ignored with a stated
//! reason, so a new or renamed field fails loudly instead of being dropped.

mod fields;
mod items;
mod npcs;
mod skills;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt::Debug;

use canastra_data::GameData;
use l2_dat::Value;
use roxmltree::Node;

/// Decoded legacy inputs: client table records and `(file name, XML text)` server documents.
pub struct Sources<'a> {
    pub weapons: &'a [Value],
    pub armors: &'a [Value],
    pub etc_items: &'a [Value],
    pub item_names: &'a [Value],
    pub skills: &'a [Value],
    pub skill_names: &'a [Value],
    pub skill_sounds: &'a [Value],
    pub npcs: &'a [Value],
    pub npc_names: &'a [Value],
    pub server_items: &'a [(String, String)],
    pub server_skills: &'a [(String, String)],
    pub server_npcs: &'a [(String, String)],
}

pub fn migrate(sources: &Sources<'_>) -> (GameData, Report) {
    let mut report = Report::default();
    let data = GameData {
        items: items::migrate(sources, &mut report),
        skills: skills::migrate(sources, &mut report),
        npcs: npcs::migrate(sources, &mut report),
    };
    (data, report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The entity could not be migrated.
    Error,
    /// The entity was migrated, but something was dropped or looks wrong.
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub subject: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// Legacy server data with no place in the model yet, by key and occurrence count.
    pub unmodeled: BTreeMap<String, usize>,
}

impl Report {
    fn push(&mut self, severity: Severity, subject: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic { severity, subject: subject.into(), message: message.into() });
    }

    fn unmodeled(&mut self, key: String) {
        *self.unmodeled.entry(key).or_default() += 1;
    }

    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == severity).count()
    }
}

/// Maps every client record and indexes it by key; failures and repeated keys are reported.
fn index<K: Ord + Debug, T>(
    table: &str,
    records: &[Value],
    map: fn(&Value) -> fields::Result<T>,
    key: fn(&T) -> K,
    duplicate: Severity,
    report: &mut Report,
) -> BTreeMap<K, T> {
    let mut out = BTreeMap::new();
    for (position, record) in records.iter().enumerate() {
        match map(record) {
            Ok(entry) => match out.entry(key(&entry)) {
                Entry::Occupied(slot) => {
                    report.push(duplicate, format!("{table} {:?}", slot.key()), "duplicate record; first one kept");
                }
                Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
            },
            Err(error) => report.push(Severity::Error, format!("{table} record {position}"), error),
        }
    }
    out
}

/// Parses each top-level `<tag>` element of every document into a map. Parse failures are
/// errors; a repeated key keeps the last definition, as the reference server does.
fn server_index<K: Ord, T>(
    documents: &[(String, String)],
    tag: &str,
    report: &mut Report,
    parse: fn(Node<'_, '_>, &mut Report) -> fields::Result<(K, T)>,
) -> BTreeMap<K, T> {
    let mut out = BTreeMap::new();
    for (file, xml) in documents {
        let document = match roxmltree::Document::parse(xml) {
            Ok(document) => document,
            Err(error) => {
                report.push(Severity::Error, file.clone(), error.to_string());
                continue;
            }
        };
        for node in document.root_element().children().filter(|node| node.has_tag_name(tag)) {
            let subject = format!("{tag} {}", node.attribute("id").unwrap_or("?"));
            match parse(node, report) {
                Ok((key, value)) => {
                    if out.insert(key, value).is_some() {
                        report.push(
                            Severity::Warning,
                            subject,
                            format!("{file}: defined more than once; last one kept"),
                        );
                    }
                }
                Err(error) => report.push(Severity::Error, subject, format!("{file}: {error}")),
            }
        }
    }
    out
}
