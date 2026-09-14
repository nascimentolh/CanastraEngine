//! Player-facing text in every supported language.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Localized(BTreeMap<Locale, String>);

impl Localized {
    pub fn en(text: impl Into<String>) -> Self {
        Self(BTreeMap::from([(Locale::En, text.into())]))
    }

    pub fn get(&self, locale: Locale) -> Option<&str> {
        self.0.get(&locale).map(String::as_str)
    }
}
