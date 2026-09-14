//! Converts legacy Lineage 2 sources into Canastra game data.
//!
//! Inputs are already decoded: client tables from `l2-dat` and server XML as text.
//! Every legacy client field is either mapped or ignored with a stated reason, so a
//! new or renamed field fails loudly instead of being dropped.

mod client;
mod fields;
mod items;
mod server;

use std::collections::BTreeMap;

pub use items::{ItemSources, migrate_items};

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
