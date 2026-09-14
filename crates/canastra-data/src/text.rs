//! Player-facing text in every supported language.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

    pub fn set(&mut self, locale: Locale, text: impl Into<String>) {
        self.0.insert(locale, text.into());
    }

    pub fn get(&self, locale: Locale) -> Option<&str> {
        self.0.get(&locale).map(String::as_str)
    }
}
