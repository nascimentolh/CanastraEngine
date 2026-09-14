//! Player-facing text in every supported language.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Locale {
    En,
    PtBr,
    Es,
    Ru,
    Zh,
    Ja,
    Ko,
}

/// Text per locale.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localized(BTreeMap<Locale, String>);

impl Localized {
    pub fn en(text: impl Into<String>) -> Self {
        Self(BTreeMap::from([(Locale::En, text.into())]))
    }

    pub fn get(&self, locale: Locale) -> Option<&str> {
        self.0.get(&locale).map(String::as_str)
    }
}
