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

/// Text per locale; English is the fallback for untranslated entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// The text for `locale`, falling back to English, then to empty.
    pub fn resolve(&self, locale: Locale) -> &str {
        self.get(locale).or_else(|| self.get(Locale::En)).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_english() {
        let mut text = Localized::en("Adena");
        text.set(Locale::Ru, "Адена");
        assert_eq!(text.resolve(Locale::Ru), "Адена");
        assert_eq!(text.resolve(Locale::PtBr), "Adena");
        assert_eq!(Localized::default().resolve(Locale::Ko), "");
    }
}
